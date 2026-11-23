//! Ponte entre a interface (síncrona) e o librqbit (assíncrono, tokio).
//!
//! A interface chama métodos simples (`resolve`, `add`, `pause`...) que disparam
//! tarefas no runtime; os resultados voltam como `EngineEvent` por um canal.
//! O estado dos torrents é lido direto da sessão a cada quadro (`snapshot`).

use std::{
    collections::HashMap,
    net::{Ipv6Addr, SocketAddr},
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::{Arc, RwLock, mpsc},
    time::Duration,
};

use eframe::egui;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ListenerOptions, Magnet, ManagedTorrent,
    Session, SessionOptions, SessionPersistenceConfig, TorrentStatsState, api::TorrentIdOrHash,
    limits::LimitsConfig,
};

use crate::settings::{Settings, data_dir};

/// Quanto tempo esperar pelas informações de um magnet antes de desistir.
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(180);

// ---------------------------------------------------------------- origem

#[derive(Clone, Debug)]
pub enum Source {
    Magnet(String),
    Url(String),
    File(PathBuf),
}

impl Source {
    /// Entende um texto colado ou um caminho: magnet, URL, arquivo .torrent ou info-hash.
    pub fn parse(text: &str) -> Option<Source> {
        let t = text.trim().trim_matches('"').trim();
        if t.is_empty() {
            return None;
        }
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("magnet:?") {
            return Some(Source::Magnet(t.to_string()));
        }
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Some(Source::Url(t.to_string()));
        }
        let is_hash = (t.len() == 40 && t.chars().all(|c| c.is_ascii_hexdigit()))
            || (t.len() == 32 && t.chars().all(|c| c.is_ascii_alphanumeric()));
        if is_hash && !Path::new(t).exists() {
            return Some(Source::Magnet(format!("magnet:?xt=urn:btih:{t}")));
        }
        let p = PathBuf::from(t);
        if p.is_file() && lower.ends_with(".torrent") {
            return Some(Source::File(p));
        }
        None
    }

    /// Nome provisório para mostrar enquanto as informações chegam.
    pub fn label(&self) -> String {
        match self {
            Source::Magnet(m) => Magnet::parse(m)
                .ok()
                .and_then(|m| m.name)
                .unwrap_or_else(|| "Link magnet".into()),
            Source::Url(u) => u.rsplit('/').next().unwrap_or(u).to_string(),
            Source::File(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }
}

// ---------------------------------------------------------------- tipos públicos

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub index: usize,
    pub path: String,
    pub len: u64,
    pub selected: bool,
}

/// Informações do torrent já resolvidas, prontas para o usuário escolher os arquivos.
#[derive(Clone)]
pub struct Resolved {
    pub name: String,
    pub files: Vec<FileEntry>,
    /// Nome da subpasta que o torrent cria (torrents com vários arquivos).
    pub subfolder: Option<String>,
    torrent_bytes: Vec<u8>,
    peers: Vec<SocketAddr>,
    trackers: Vec<String>,
}

pub enum EngineEvent {
    Ready,
    StartFailed(String),
    Resolved { req: u64, result: Result<Resolved, String> },
    AlreadyAdded { req: u64, name: String },
    Added { name: String },
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Checking,
    Downloading,
    Seeding,
    Done,
    Paused,
    Error,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub id: usize,
    pub name: String,
    pub status: Status,
    pub error: Option<String>,
    pub progress: f32,
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub down_bps: f64,
    pub up_bps: f64,
    pub eta_secs: Option<u64>,
    pub peers: u32,
    pub seeds: u32,
    pub uploaded: u64,
    pub finished: bool,
    /// O que abrir no Explorer: a pasta do torrent ou o arquivo único.
    pub open_path: PathBuf,
    pub folder: PathBuf,
    pub info_hash: String,
}

impl Row {
    pub fn ratio(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.uploaded as f32 / self.total_bytes as f32
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct Totals {
    pub down_bps: f64,
    pub up_bps: f64,
    pub peers: u32,
    pub dht_nodes: usize,
}

// ---------------------------------------------------------------- engine

pub struct Engine {
    rt: tokio::runtime::Runtime,
    session: Arc<RwLock<Option<Arc<Session>>>>,
    tx: mpsc::Sender<EngineEvent>,
    rx: mpsc::Receiver<EngineEvent>,
    ctx: egui::Context,
    resolving: HashMap<u64, tokio::task::JoinHandle<()>>,
    pub port: Option<u16>,
    port_slot: Arc<RwLock<Option<u16>>>,
}

impl Engine {
    pub fn start(ctx: egui::Context, settings: &Settings) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("mare-rt")
            .build()
            .expect("não foi possível iniciar o runtime");

        let (tx, rx) = mpsc::channel();
        let session: Arc<RwLock<Option<Arc<Session>>>> = Default::default();
        let port_slot: Arc<RwLock<Option<u16>>> = Default::default();

        let downloads = settings.pasta_downloads.clone();
        let limits = limits_config(settings.limite_download_kbps, settings.limite_upload_kbps);
        let port = settings.porta;
        {
            let session = session.clone();
            let tx = tx.clone();
            let ctx = ctx.clone();
            let port_slot = port_slot.clone();
            rt.spawn(async move {
                let _ = tokio::fs::create_dir_all(&downloads).await;
                // Tenta a porta escolhida; se estiver ocupada, deixa o sistema escolher uma.
                let mut result = Session::new_with_opts(
                    downloads.clone(),
                    session_options(port, limits),
                )
                .await;
                if result.is_err() && port != 0 {
                    result =
                        Session::new_with_opts(downloads, session_options(0, limits)).await;
                }
                match result {
                    Ok(s) => {
                        *port_slot.write().unwrap() = s.announce_port();
                        *session.write().unwrap() = Some(s);
                        let _ = tx.send(EngineEvent::Ready);
                    }
                    Err(e) => {
                        let _ = tx.send(EngineEvent::StartFailed(format!("{e:#}")));
                    }
                }
                ctx.request_repaint();
            });
        }

        Self {
            rt,
            session,
            tx,
            rx,
            ctx,
            resolving: HashMap::new(),
            port: None,
            port_slot,
        }
    }

    fn session(&self) -> Option<Arc<Session>> {
        self.session.read().unwrap().clone()
    }

    pub fn is_ready(&self) -> bool {
        self.session.read().unwrap().is_some()
    }

    pub fn poll_events(&mut self) -> Vec<EngineEvent> {
        if self.port.is_none() {
            self.port = *self.port_slot.read().unwrap();
        }
        let events: Vec<_> = self.rx.try_iter().collect();
        for e in &events {
            if let EngineEvent::Resolved { req, .. } | EngineEvent::AlreadyAdded { req, .. } = e {
                self.resolving.remove(req);
            }
        }
        events
    }

    /// Busca as informações do torrent (lista de arquivos) sem começar o download.
    pub fn resolve(&mut self, req: u64, source: Source) {
        let session_slot = self.session.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let handle = self.rt.spawn(async move {
            let send = |ev| {
                let _ = tx.send(ev);
                ctx.request_repaint();
            };
            // A sessão pode ainda estar subindo: espera um pouco.
            let mut session = None;
            for _ in 0..100 {
                if let Some(s) = session_slot.read().unwrap().clone() {
                    session = Some(s);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let Some(session) = session else {
                send(EngineEvent::Resolved {
                    req,
                    result: Err("O motor de torrent não iniciou.".into()),
                });
                return;
            };

            let mut trackers = Vec::new();
            let add = match &source {
                Source::Magnet(m) => {
                    if let Ok(mg) = Magnet::parse(m) {
                        trackers = mg.trackers.clone();
                    }
                    AddTorrent::from_url(m.clone())
                }
                Source::Url(u) => AddTorrent::from_url(u.clone()),
                Source::File(p) => match tokio::fs::read(p).await {
                    Ok(b) => AddTorrent::from_bytes(b),
                    Err(e) => {
                        send(EngineEvent::Resolved {
                            req,
                            result: Err(format!("Não consegui ler o arquivo: {e}")),
                        });
                        return;
                    }
                },
            };

            let opts = AddTorrentOptions {
                list_only: true,
                ..Default::default()
            };
            let res = tokio::time::timeout(RESOLVE_TIMEOUT, session.add_torrent(add, Some(opts))).await;
            let ev = match res {
                Err(_) => EngineEvent::Resolved {
                    req,
                    result: Err("Ninguém respondeu com as informações desse torrent. \
                                 Pode não haver ninguém compartilhando agora."
                        .into()),
                },
                Ok(Err(e)) => EngineEvent::Resolved {
                    req,
                    result: Err(friendly_error(&e)),
                },
                Ok(Ok(AddTorrentResponse::AlreadyManaged(_, h)))
                | Ok(Ok(AddTorrentResponse::Added(_, h))) => EngineEvent::AlreadyAdded {
                    req,
                    name: h.name().unwrap_or_else(|| "Esse torrent".into()),
                },
                Ok(Ok(AddTorrentResponse::ListOnly(r))) => {
                    let selected_only = r.only_files.clone();
                    let files = r
                        .info
                        .iter_file_details()
                        .enumerate()
                        .filter(|(_, fd)| !fd.attrs().padding)
                        .map(|(i, fd)| FileEntry {
                            index: i,
                            path: fd.filename.to_pathbuf().to_string_lossy().into_owned(),
                            len: fd.len,
                            selected: selected_only.as_ref().is_none_or(|s| s.contains(&i)),
                        })
                        .collect::<Vec<_>>();
                    let name = r
                        .info
                        .name()
                        .map(|n| n.into_owned())
                        .unwrap_or_else(|| source.label());
                    // Quando o librqbit decide criar uma subpasta, o nome dela é o último
                    // componente da pasta de saída que ele calculou.
                    let has_dir = r.info.iter_file_details().count() > 1
                        || r.info.info().files.is_some();
                    let subfolder = if has_dir {
                        r.output_folder
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                    } else {
                        None
                    };
                    EngineEvent::Resolved {
                        req,
                        result: Ok(Resolved {
                            name,
                            files,
                            subfolder,
                            torrent_bytes: r.torrent_bytes.to_vec(),
                            peers: r.seen_peers,
                            trackers,
                        }),
                    }
                }
            };
            send(ev);
        });
        self.resolving.insert(req, handle);
    }

    pub fn cancel_resolve(&mut self, req: u64) {
        if let Some(h) = self.resolving.remove(&req) {
            h.abort();
        }
    }

    /// Começa o download com os arquivos escolhidos.
    pub fn add(&self, r: Resolved, folder: PathBuf, paused: bool) {
        let Some(session) = self.session() else { return };
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        self.rt.spawn(async move {
            let selected: Vec<usize> = r.files.iter().filter(|f| f.selected).map(|f| f.index).collect();
            let all = selected.len() == r.files.len();
            let output = match &r.subfolder {
                Some(sub) => folder.join(sub),
                None => folder,
            };
            let opts = AddTorrentOptions {
                paused,
                only_files: if all { None } else { Some(selected) },
                output_folder: Some(output.to_string_lossy().into_owned()),
                overwrite: true,
                initial_peers: (!r.peers.is_empty()).then(|| r.peers.clone()),
                trackers: (!r.trackers.is_empty()).then(|| r.trackers.clone()),
                ..Default::default()
            };
            let ev = match session
                .add_torrent(AddTorrent::from_bytes(r.torrent_bytes.clone()), Some(opts))
                .await
            {
                Ok(_) => EngineEvent::Added { name: r.name.clone() },
                Err(e) => EngineEvent::Error(format!("Não deu para adicionar “{}”: {}", r.name, friendly_error(&e))),
            };
            let _ = tx.send(ev);
            ctx.request_repaint();
        });
    }

    fn with_handle(
        &self,
        id: usize,
        f: impl FnOnce(Arc<Session>, Arc<ManagedTorrent>) -> futures_like::BoxFut + Send + 'static,
    ) {
        let Some(session) = self.session() else { return };
        let Some(handle) = session.get(TorrentIdOrHash::Id(id)) else { return };
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        self.rt.spawn(async move {
            if let Err(e) = f(session, handle).await {
                let _ = tx.send(EngineEvent::Error(friendly_error(&e)));
            }
            ctx.request_repaint();
        });
    }

    pub fn pause(&self, id: usize) {
        self.with_handle(id, |s, h| Box::pin(async move { s.pause(&h).await }));
    }

    pub fn resume(&self, id: usize) {
        self.with_handle(id, |s, h| Box::pin(async move { s.unpause(&h).await }));
    }

    pub fn delete(&self, id: usize, delete_files: bool) {
        self.with_handle(id, move |s, _h| {
            Box::pin(async move { s.delete(TorrentIdOrHash::Id(id), delete_files).await })
        });
    }

    pub fn set_limits(&self, down_kbps: u32, up_kbps: u32) {
        if let Some(s) = self.session() {
            let cfg = limits_config(down_kbps, up_kbps);
            s.ratelimits.set_download_bps(cfg.download_bps);
            s.ratelimits.set_upload_bps(cfg.upload_bps);
        }
    }

    pub fn snapshot(&self) -> (Vec<Row>, Totals) {
        let Some(session) = self.session() else {
            return (Vec::new(), Totals::default());
        };
        let mut rows: Vec<Row> = session.with_torrents(|it| it.map(|(id, h)| make_row(id, h)).collect());
        rows.sort_by_key(|r| std::cmp::Reverse(r.id));

        let st = session.stats_snapshot();
        let totals = Totals {
            down_bps: st.download_speed.as_bytes() as f64,
            up_bps: st.upload_speed.as_bytes() as f64,
            peers: st.peers.live,
            dht_nodes: session
                .get_dht()
                .map(|d| {
                    let s = d.stats();
                    s.routing_table_size + s.routing_table_size_v6
                })
                .unwrap_or(0),
        };
        (rows, totals)
    }

    /// Pausa tudo e dá tempo para o librqbit salvar o estado.
    pub fn shutdown(&self) {
        if let Some(s) = self.session() {
            self.rt.block_on(async move {
                let _ = tokio::time::timeout(Duration::from_secs(3), s.stop()).await;
            });
        }
    }
}

mod futures_like {
    pub type BoxFut = std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>;
}

fn session_options(port: u16, limits: LimitsConfig) -> SessionOptions {
    SessionOptions {
        fastresume: true,
        persistence: Some(SessionPersistenceConfig::Json {
            folder: Some(data_dir().join("sessao")),
        }),
        listen: Some(ListenerOptions {
            listen_addr: (Ipv6Addr::UNSPECIFIED, port).into(),
            enable_upnp_port_forwarding: true,
            ..Default::default()
        }),
        ratelimits: limits,
        client_name_and_version: Some(format!("Mare/{}", env!("CARGO_PKG_VERSION"))),
        ..Default::default()
    }
}

fn limits_config(down_kbps: u32, up_kbps: u32) -> LimitsConfig {
    LimitsConfig {
        download_bps: NonZeroU32::new(down_kbps.saturating_mul(1024)),
        upload_bps: NonZeroU32::new(up_kbps.saturating_mul(1024)),
    }
}

fn make_row(id: usize, h: &Arc<ManagedTorrent>) -> Row {
    let st = h.stats();
    let status = match st.state {
        TorrentStatsState::Initializing { .. } => Status::Checking,
        TorrentStatsState::Live if st.finished => Status::Seeding,
        TorrentStatsState::Live => Status::Downloading,
        TorrentStatsState::Paused if st.finished => Status::Done,
        TorrentStatsState::Paused => Status::Paused,
        TorrentStatsState::Error => Status::Error,
    };
    let (down, up, peers, seeds) = match &st.live {
        Some(l) => (
            l.download_speed.as_bytes() as f64,
            l.upload_speed.as_bytes() as f64,
            l.snapshot.peer_stats.live,
            l.snapshot.peer_stats.live_seeders,
        ),
        None => (0.0, 0.0, 0, 0),
    };
    let remaining = st.total_bytes.saturating_sub(st.progress_bytes);
    let eta = (status == Status::Downloading && down > 512.0).then(|| (remaining as f64 / down) as u64);
    let progress = if st.total_bytes > 0 {
        st.progress_bytes as f32 / st.total_bytes as f32
    } else {
        0.0
    };

    let folder = h.output_folder().to_path_buf();
    // Torrent de um arquivo só: abrir o Explorer já com o arquivo selecionado.
    let single = h
        .with_metadata(|m| (m.file_infos.len() == 1).then(|| m.file_infos[0].relative_filename.clone()))
        .ok()
        .flatten();
    let open_path = match single {
        Some(rel) => folder.join(rel),
        None => folder.clone(),
    };

    Row {
        id,
        name: h.name().unwrap_or_else(|| format!("Torrent {id}")),
        status,
        error: st.error.clone().map(|e| e.lines().next().unwrap_or_default().to_string()),
        progress,
        done_bytes: st.progress_bytes,
        total_bytes: st.total_bytes,
        down_bps: down,
        up_bps: up,
        eta_secs: eta,
        peers,
        seeds,
        uploaded: st.uploaded_bytes,
        finished: st.finished,
        open_path,
        folder,
        info_hash: h.info_hash().as_string(),
    }
}

/// Info-hash (hex) de um link magnet, para saber se ele já está na lista.
pub fn magnet_hash(text: &str) -> Option<String> {
    Magnet::parse(text.trim()).ok()?.as_id20().map(|h| h.as_string())
}

/// Deixa as mensagens de erro mais curtas e em português quando dá.
fn friendly_error(e: &anyhow::Error) -> String {
    let msg = format!("{e:#}");
    let low = msg.to_lowercase();
    if low.contains("not a valid magnet") {
        "Esse link magnet não é válido.".into()
    } else if low.contains("error decoding") || low.contains("bencode") || low.contains("torrent_from_bytes") {
        "Esse arquivo não parece ser um .torrent válido.".into()
    } else if low.contains("error downloading") || low.contains("dns") || low.contains("connect") && low.contains("http") {
        format!("Não consegui baixar o .torrent: {msg}")
    } else {
        msg
    }
}
