//! A janela do Maré.

use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    time::{Duration, Instant},
};

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Frame, Id, Layout, Margin, RichText, ScrollArea,
    Sense, Stroke, TextEdit, Ui, vec2,
};

use crate::{
    engine::{Engine, EngineEvent, Resolved, Row, Source, Status, Totals, magnet_hash},
    settings::{Settings, Tema},
    theme::{self, Palette, semibold},
    util, widgets,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Downloading,
    Seeding,
    Done,
    Paused,
    Errors,
}

impl Filter {
    fn matches(self, s: Status) -> bool {
        match self {
            Filter::All => true,
            Filter::Downloading => matches!(s, Status::Downloading | Status::Checking),
            Filter::Seeding => s == Status::Seeding,
            Filter::Done => matches!(s, Status::Done | Status::Seeding),
            Filter::Paused => s == Status::Paused,
            Filter::Errors => s == Status::Error,
        }
    }
}

enum AddStage {
    Input { text: String, error: Option<String>, focus: bool },
    Resolving { req: u64, label: String, source: Source },
    Files(Box<Resolved>),
    Failed { message: String, source: Option<Source> },
    Already(String),
}

struct AddDialog {
    stage: AddStage,
    folder: PathBuf,
    paused: bool,
    error: Option<String>,
}

struct DeleteDialog {
    id: usize,
    name: String,
    delete_files: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToastKind {
    Info,
    Success,
    Error,
}

struct Toast {
    text: String,
    kind: ToastKind,
    until: Instant,
}

pub struct MareApp {
    engine: Engine,
    settings: Settings,
    pal: Palette,
    applied_dark: Option<bool>,

    rows: Vec<Row>,
    totals: Totals,
    last_snapshot: Instant,
    /// Torrents que já existiam quando a sessão ficou pronta.
    startup_ids: Option<HashSet<usize>>,
    /// Torrents vistos pelo menos uma vez.
    seen: HashSet<usize>,
    /// Torrents incompletos que merecem aviso quando terminarem.
    watching: HashSet<usize>,

    filter: Filter,
    search: String,

    add: Option<AddDialog>,
    queue: VecDeque<Source>,
    delete: Option<DeleteDialog>,
    show_settings: bool,
    toasts: Vec<Toast>,
    next_req: u64,

    start_error: Option<String>,
    clipboard_checked: Option<String>,
    clipboard_offer: Option<(String, String)>,
    was_focused: bool,
    save_at: Option<Instant>,
}

impl MareApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<Source>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let settings = Settings::load();
        let engine = Engine::start(cc.egui_ctx.clone(), &settings);
        let mut app = Self {
            engine,
            pal: Palette::dark(),
            applied_dark: None,
            settings,
            rows: Vec::new(),
            totals: Totals::default(),
            last_snapshot: Instant::now() - Duration::from_secs(5),
            startup_ids: None,
            seen: HashSet::new(),
            watching: HashSet::new(),
            filter: Filter::All,
            search: String::new(),
            add: None,
            queue: VecDeque::new(),
            delete: None,
            show_settings: false,
            toasts: Vec::new(),
            next_req: 1,
            start_error: None,
            clipboard_checked: None,
            clipboard_offer: None,
            was_focused: false,
            save_at: None,
        };
        if let Some(src) = initial {
            app.queue.push_back(src);
        }
        app
    }

    // ------------------------------------------------------------ estado

    fn update_theme(&mut self, ctx: &egui::Context) {
        let dark = match self.settings.tema {
            Tema::Escuro => true,
            Tema::Claro => false,
            Tema::Sistema => ctx.system_theme().is_none_or(|t| t == egui::Theme::Dark),
        };
        if self.applied_dark != Some(dark) {
            self.pal = if dark { Palette::dark() } else { Palette::light() };
            theme::apply(ctx, &self.pal);
            self.applied_dark = Some(dark);
        }
    }

    fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toasts.push(Toast {
            text: text.into(),
            kind,
            until: Instant::now() + Duration::from_secs(if kind == ToastKind::Error { 7 } else { 4 }),
        });
    }

    fn save_soon(&mut self) {
        self.save_at = Some(Instant::now() + Duration::from_millis(600));
    }

    fn handle_events(&mut self) {
        for ev in self.engine.poll_events() {
            match ev {
                EngineEvent::Ready => {}
                EngineEvent::StartFailed(e) => self.start_error = Some(e),
                EngineEvent::Resolved { req, result } => {
                    let Some(dlg) = self.add.as_mut() else { continue };
                    let AddStage::Resolving { req: cur, source, .. } = &dlg.stage else { continue };
                    if *cur != req {
                        continue;
                    }
                    let source = source.clone();
                    dlg.stage = match result {
                        Ok(r) => AddStage::Files(Box::new(r)),
                        Err(message) => AddStage::Failed { message, source: Some(source) },
                    };
                }
                EngineEvent::AlreadyAdded { req, name } => {
                    if let Some(dlg) = self.add.as_mut()
                        && matches!(&dlg.stage, AddStage::Resolving { req: cur, .. } if *cur == req)
                    {
                        dlg.stage = AddStage::Already(name);
                    }
                }
                EngineEvent::Added { name } => self.toast(ToastKind::Info, format!("Baixando “{name}”")),
                EngineEvent::Error(e) => self.toast(ToastKind::Error, e),
            }
        }
    }

    fn refresh(&mut self) {
        if self.last_snapshot.elapsed() < Duration::from_millis(400) {
            return;
        }
        self.last_snapshot = Instant::now();
        let (rows, totals) = self.engine.snapshot();
        self.totals = totals;

        // Aviso de "concluído": só para torrents que vimos incompletos nesta sessão, ou que
        // foram adicionados agora. Ao abrir o app, os que já estavam prontos passam pela
        // verificação de arquivos e não podem gerar aviso de novo.
        if self.engine.is_ready() && self.startup_ids.is_none() {
            self.startup_ids = Some(rows.iter().map(|r| r.id).collect());
        }
        let mut finished_now = Vec::new();
        for r in &rows {
            let is_new = self.startup_ids.as_ref().is_some_and(|s| !s.contains(&r.id));
            let first_sight = self.seen.insert(r.id);
            if !r.finished && (r.status != Status::Checking || is_new) {
                self.watching.insert(r.id);
            }
            let tiny_new = is_new && first_sight;
            if r.finished && r.total_bytes > 0 && (self.watching.remove(&r.id) || tiny_new) {
                finished_now.push(r.clone());
            }
        }
        for r in finished_now {
            self.on_finished(&r);
        }

        // O magnet oferecido pela área de transferência já foi adicionado? Some a faixa.
        if let Some((text, _)) = &self.clipboard_offer
            && let Some(hash) = magnet_hash(text)
            && rows.iter().any(|r| r.info_hash.eq_ignore_ascii_case(&hash))
        {
            self.clipboard_offer = None;
        }
        self.rows = rows;
    }

    fn on_finished(&mut self, r: &Row) {
        self.toast(ToastKind::Success, format!("Concluído: {}", r.name));
        if self.settings.notificar {
            let name = r.name.clone();
            std::thread::spawn(move || {
                let _ = notify_rust::Notification::new()
                    .summary("Download concluído")
                    .body(&name)
                    .appname("Maré")
                    .show();
            });
        }
        if !self.settings.semear_depois && r.status == Status::Seeding {
            self.engine.pause(r.id);
        }
    }

    // ------------------------------------------------------------ entrada

    fn open_add(&mut self, source: Option<Source>) {
        let folder = self.settings.pasta_downloads.clone();
        let stage = match source {
            Some(src) => {
                let req = self.next_req;
                self.next_req += 1;
                self.engine.resolve(req, src.clone());
                AddStage::Resolving { req, label: src.label(), source: src }
            }
            None => AddStage::Input { text: String::new(), error: None, focus: true },
        };
        self.add = Some(AddDialog { stage, folder, paused: false, error: None });
    }

    fn enqueue(&mut self, src: Source) {
        if self.add.is_none() {
            self.open_add(Some(src));
        } else {
            self.queue.push_back(src);
        }
    }

    fn close_add(&mut self) {
        if let Some(AddDialog { stage: AddStage::Resolving { req, .. }, .. }) = &self.add {
            self.engine.cancel_resolve(*req);
        }
        self.add = None;
        if let Some(next) = self.queue.pop_front() {
            self.open_add(Some(next));
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // Arquivos arrastados para a janela.
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        let mut ignored = 0;
        for f in dropped {
            let path = f.path().to_path_buf();
            match Source::parse(&path.to_string_lossy()) {
                Some(src) => self.enqueue(src),
                None => ignored += 1,
            }
        }
        if ignored > 0 {
            self.toast(ToastKind::Error, "Só dá para soltar arquivos .torrent.");
        }

        // Ctrl+V fora de um campo de texto.
        let nothing_focused = ctx.memory(|m| m.focused().is_none());
        if nothing_focused && self.add.is_none() {
            let pasted: Vec<String> = ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Paste(t) => Some(t.clone()),
                        _ => None,
                    })
                    .collect()
            });
            for t in pasted {
                match Source::parse(&t) {
                    Some(src) => self.enqueue(src),
                    None => self.toast(ToastKind::Error, "O texto colado não é um link magnet nem um .torrent."),
                }
            }
        }

        // Link magnet copiado: oferece para adicionar quando a janela ganha foco.
        let focused = ctx.input(|i| i.focused);
        if focused && !self.was_focused && self.engine.is_ready() {
            self.check_clipboard();
        }
        self.was_focused = focused && self.engine.is_ready();
    }

    fn check_clipboard(&mut self) {
        let Ok(mut cb) = arboard::Clipboard::new() else { return };
        let Ok(text) = cb.get_text() else { return };
        let text = text.trim().to_string();
        if self.clipboard_checked.as_deref() == Some(text.as_str()) {
            return;
        }
        self.clipboard_checked = Some(text.clone());
        if !text.to_ascii_lowercase().starts_with("magnet:?") {
            return;
        }
        let Some(hash) = magnet_hash(&text) else { return };
        if self.rows.iter().any(|r| r.info_hash.eq_ignore_ascii_case(&hash)) {
            return;
        }
        let label = Source::Magnet(text.clone()).label();
        self.clipboard_offer = Some((text, label));
    }

    // ------------------------------------------------------------ desenho

    fn top_bar(&mut self, ui: &mut Ui) {
        let p = self.pal;
        let resp = egui::Panel::top("topo")
            .frame(Frame::new().fill(p.panel).inner_margin(Margin::symmetric(16, 10)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    widgets::logo(ui, &p);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon(ui, &p, "⚙", "Configurações").clicked() {
                            self.show_settings = true;
                        }
                        if widgets::primary(ui, &p, "+  Adicionar").clicked() {
                            self.open_add(None);
                        }
                        ui.add_space(4.0);
                        ui.add(
                            TextEdit::singleline(&mut self.search)
                                .hint_text("🔍  Buscar nos downloads")
                                .desired_width(240.0)
                                .margin(Margin::symmetric(10, 7)),
                        );
                    });
                });
            });
        let r = resp.response.rect;
        ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(1.0, p.border));
    }

    fn side_bar(&mut self, ui: &mut Ui) {
        let p = self.pal;
        let count = |f: Filter| self.rows.iter().filter(|r| f.matches(r.status)).count();
        let counts = [
            (Filter::All, "☰", "Todos", self.rows.len()),
            (Filter::Downloading, "⬇", "Baixando", count(Filter::Downloading)),
            (Filter::Seeding, "⬆", "Semeando", count(Filter::Seeding)),
            (Filter::Done, "✔", "Concluídos", count(Filter::Done)),
            (Filter::Paused, "⏸", "Pausados", count(Filter::Paused)),
        ];
        let errors = count(Filter::Errors);

        let resp = egui::Panel::left("lateral")
            .resizable(false)
            .exact_size(200.0)
            .frame(Frame::new().fill(p.panel).inner_margin(Margin::symmetric(10, 14)))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (f, glyph, label, n) in counts {
                    if widgets::nav_item(ui, &p, glyph, label, n, self.filter == f).clicked() {
                        self.filter = f;
                    }
                }
                if errors > 0
                    && widgets::nav_item(ui, &p, "⚠", "Com erro", errors, self.filter == Filter::Errors).clicked()
                {
                    self.filter = Filter::Errors;
                }

                ui.add_space(18.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    widgets::section(ui, &p, "LIMITE DE VELOCIDADE");
                });
                let mut changed = false;
                changed |= limit_slider(ui, &p, "⬇  Download", &mut self.settings.limite_download_kbps);
                changed |= limit_slider(ui, &p, "⬆  Upload", &mut self.settings.limite_upload_kbps);
                if changed {
                    self.engine
                        .set_limits(self.settings.limite_download_kbps, self.settings.limite_upload_kbps);
                    self.save_soon();
                }
            });
        let r = resp.response.rect;
        ui.painter().vline(r.right(), r.y_range(), Stroke::new(1.0, p.border));
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        let p = self.pal;
        let t = self.totals;
        let resp = egui::Panel::bottom("status")
            .frame(Frame::new().fill(p.panel).inner_margin(Margin::symmetric(16, 7)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 18.0;
                    let small = |s: String, c: Color32| RichText::new(s).size(12.5).color(c);
                    ui.label(small(format!("⬇ {}", util::speed(t.down_bps)), p.accent));
                    ui.label(small(format!("⬆ {}", util::speed(t.up_bps)), p.success));
                    let conectados = if t.peers == 1 { "par conectado" } else { "pares conectados" };
                    ui.label(small(format!("{} {conectados}", t.peers), p.muted));
                    if t.dht_nodes > 0 {
                        ui.label(small(format!("DHT {} nós", t.dht_nodes), p.muted));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !self.engine.is_ready() && self.start_error.is_none() {
                            ui.label(small("Iniciando…".into(), p.muted));
                            ui.add(egui::Spinner::new().size(12.0));
                        } else if let Some(port) = self.engine.port {
                            ui.label(small(format!("Porta {port}"), p.faint));
                        }
                        let lim = self.settings.limite_download_kbps;
                        if lim > 0 {
                            ui.label(small(format!("Limite ⬇ {}", util::speed(lim as f64 * 1024.0)), p.warning));
                        }
                    });
                });
            });
        let r = resp.response.rect;
        ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, p.border));
    }

    fn central(&mut self, ui: &mut Ui) {
        let p = self.pal;
        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(20, 16)))
            .show(ui, |ui| {
                self.banners(ui);
                if self.rows.is_empty() {
                    self.empty_state(ui);
                    return;
                }
                let q = self.search.trim().to_lowercase();
                let visible: Vec<Row> = self
                    .rows
                    .iter()
                    .filter(|r| self.filter.matches(r.status))
                    .filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q))
                    .cloned()
                    .collect();
                if visible.is_empty() {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Nada por aqui").font(semibold(16.0)).color(p.muted));
                        ui.label(RichText::new("Nenhum download combina com esse filtro.").color(p.faint));
                    });
                    return;
                }
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    for r in &visible {
                        self.torrent_card(ui, r);
                    }
                    ui.add_space(8.0);
                });
            });
    }

    fn banners(&mut self, ui: &mut Ui) {
        let p = self.pal;
        if let Some(err) = &self.start_error {
            Frame::new()
                .fill(p.danger_soft)
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("O motor de torrent não iniciou").font(semibold(14.0)).color(p.danger));
                    ui.label(RichText::new(err).size(12.5).color(p.danger));
                });
            ui.add_space(10.0);
        }
        let mut accept = None;
        let mut dismiss = false;
        if let Some((text, label)) = &self.clipboard_offer {
            Frame::new()
                .fill(p.accent_soft)
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(14, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔗").color(p.accent));
                        ui.label(RichText::new("Link magnet copiado:").color(p.muted));
                        ui.add(egui::Label::new(RichText::new(label).font(semibold(14.0)).color(p.text)).truncate());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::secondary(ui, "Ignorar").clicked() {
                                dismiss = true;
                            }
                            if widgets::primary(ui, &p, "Adicionar").clicked() {
                                accept = Some(text.clone());
                            }
                        });
                    });
                });
            ui.add_space(10.0);
        }
        if let Some(t) = accept {
            self.clipboard_offer = None;
            self.enqueue(Source::Magnet(t));
        } else if dismiss {
            self.clipboard_offer = None;
        }
    }

    fn empty_state(&mut self, ui: &mut Ui) {
        let p = self.pal;
        let avail = ui.available_rect_before_wrap();
        let size = vec2(avail.width().min(520.0), 260.0);
        let rect = egui::Rect::from_center_size(avail.center() - vec2(0.0, 30.0), size);
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(18), p.panel);
        let border = egui::Shape::dashed_line(
            &rounded_points(rect.shrink(1.0), 18.0),
            Stroke::new(1.5, p.border),
            8.0,
            6.0,
        );
        painter.extend(border);
        let mark = egui::Rect::from_center_size(rect.center() - vec2(0.0, 62.0), vec2(56.0, 56.0));
        widgets::paint_mark(painter, mark, p.accent_soft, p.accent);
        painter.text(rect.center() + vec2(0.0, -8.0), Align2::CENTER_CENTER, "Arraste um arquivo .torrent para cá", semibold(17.0), p.text);
        painter.text(rect.center() + vec2(0.0, 18.0), Align2::CENTER_CENTER, "ou cole um link magnet com Ctrl+V", egui::FontId::proportional(14.0), p.muted);
        let btn = egui::Rect::from_center_size(rect.center() + vec2(0.0, 66.0), vec2(170.0, 34.0));
        let clicked = ui
            .scope_builder(egui::UiBuilder::new().max_rect(btn), |ui| {
                ui.centered_and_justified(|ui| widgets::primary(ui, &p, "+  Adicionar torrent").clicked()).inner
            })
            .inner;
        if clicked {
            self.open_add(None);
        }
    }

    fn torrent_card(&mut self, ui: &mut Ui, r: &Row) {
        let p = self.pal;
        let (strong, soft) = p.status(r.status);
        // O clique no cartão (duplo clique, menu do botão direito) é registrado antes do
        // conteúdo, para que os botões de dentro continuem recebendo os próprios cliques.
        let builder = egui::UiBuilder::new().id_salt(("card", r.id)).sense(Sense::click());
        let card = ui.scope_builder(builder, |ui| widgets::card_frame(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 7.0;
            egui::Sides::new().truncate().show(
                ui,
                |ui| {
                    ui.label(RichText::new(icon_for(&r.name)).size(17.0).color(p.muted));
                    ui.add(egui::Label::new(RichText::new(&r.name).font(semibold(15.0)).color(p.text)).truncate())
                        .on_hover_text(&r.name);
                },
                |ui| {
                    if widgets::icon(ui, &p, "🗑", "Remover").clicked() {
                        self.delete = Some(DeleteDialog { id: r.id, name: r.name.clone(), delete_files: false });
                    }
                    if widgets::icon(ui, &p, "📂", "Abrir pasta").clicked() {
                        open_in_explorer(&r.open_path, &r.folder);
                    }
                    match r.status {
                        Status::Paused | Status::Done | Status::Error => {
                            let tip = if r.status == Status::Done { "Voltar a semear" } else { "Retomar" };
                            if widgets::icon(ui, &p, "▶", tip).clicked() {
                                self.engine.resume(r.id);
                            }
                        }
                        Status::Downloading | Status::Seeding => {
                            if widgets::icon(ui, &p, "⏸", "Pausar").clicked() {
                                self.engine.pause(r.id);
                            }
                        }
                        Status::Checking => {}
                    }
                    ui.add_space(4.0);
                    widgets::badge(ui, status_label(r.status), strong, soft);
                },
            );

            let bar_color = match r.status {
                Status::Checking => p.faint,
                Status::Paused => p.warning,
                _ => strong,
            };
            widgets::progress(ui, r.progress, bar_color, p.track);

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                let m = |s: String| RichText::new(s).size(12.5).color(p.muted);
                let size_line = format!(
                    "{} de {} · {}",
                    util::bytes(r.done_bytes),
                    util::bytes(r.total_bytes),
                    util::percent(r.progress)
                );
                match r.status {
                    Status::Downloading => {
                        ui.label(m(size_line));
                        ui.label(RichText::new(format!("⬇ {}", util::speed(r.down_bps))).size(12.5).color(p.accent));
                        ui.label(m(format!("⬆ {}", util::speed(r.up_bps))));
                        if let Some(eta) = r.eta_secs {
                            ui.label(m(format!("falta {}", util::duration(eta))));
                        } else if r.peers == 0 {
                            ui.label(m("procurando pares…".into()));
                        }
                        ui.label(m(format!("{} ({} seeds)", pares(r.peers), r.seeds)));
                    }
                    Status::Seeding => {
                        ui.label(m(util::bytes(r.total_bytes)));
                        ui.label(RichText::new(format!("⬆ {}", util::speed(r.up_bps))).size(12.5).color(p.success));
                        ui.label(m(format!("razão {}", util::decimal(r.ratio() as f64, 2))));
                        ui.label(m(pares(r.peers)));
                    }
                    Status::Done => {
                        ui.label(m(format!("{} · pronto", util::bytes(r.total_bytes))));
                        ui.label(m(format!("📁 {}", r.folder.display())));
                    }
                    Status::Paused => {
                        ui.label(m(size_line));
                    }
                    Status::Checking => {
                        ui.label(m(format!("Verificando arquivos… {}", util::percent(r.progress))));
                    }
                    Status::Error => {
                        ui.label(
                            RichText::new(r.error.clone().unwrap_or_else(|| "Erro desconhecido".into()))
                                .size(12.5)
                                .color(p.danger),
                        );
                    }
                }
            });
        }))
        .response;

        if card.double_clicked() {
            open_in_explorer(&r.open_path, &r.folder);
        }
        card.context_menu(|ui| {
            ui.set_min_width(180.0);
            match r.status {
                Status::Downloading | Status::Seeding => {
                    if ui.button("⏸  Pausar").clicked() {
                        self.engine.pause(r.id);
                        ui.close();
                    }
                }
                Status::Paused | Status::Done | Status::Error => {
                    if ui.button("▶  Retomar").clicked() {
                        self.engine.resume(r.id);
                        ui.close();
                    }
                }
                Status::Checking => {}
            }
            if ui.button("📂  Abrir pasta").clicked() {
                open_in_explorer(&r.open_path, &r.folder);
                ui.close();
            }
            if ui.button("📋  Copiar nome").clicked() {
                ui.ctx().copy_text(r.name.clone());
                ui.close();
            }
            ui.separator();
            if ui.button("🗑  Remover…").clicked() {
                self.delete = Some(DeleteDialog { id: r.id, name: r.name.clone(), delete_files: false });
                ui.close();
            }
        });
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.toasts.retain(|t| t.until > now);
        if self.toasts.is_empty() {
            return;
        }
        let p = self.pal;
        egui::Area::new(Id::unique("toasts"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-16.0, -46.0))
            .order(egui::Order::Foreground)
            .fade_in(false)
            .show(ctx, |ui| {
                ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
                    for t in self.toasts.iter().rev().take(4) {
                        let (fg, glyph) = match t.kind {
                            ToastKind::Info => (p.accent, "ℹ"),
                            ToastKind::Success => (p.success, "✔"),
                            ToastKind::Error => (p.danger, "⚠"),
                        };
                        Frame::new()
                            .fill(p.card)
                            .stroke(Stroke::new(1.0, p.border))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::symmetric(14, 10))
                            .shadow(egui::Shadow {
                                offset: [0, 4],
                                blur: 16,
                                spread: 0,
                                color: Color32::from_black_alpha(if p.dark { 120 } else { 30 }),
                            })
                            .show(ui, |ui| {
                                ui.set_max_width(360.0);
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    ui.label(RichText::new(glyph).color(fg));
                                    ui.add(egui::Label::new(RichText::new(&t.text).color(p.text)).wrap());
                                });
                            });
                        ui.add_space(6.0);
                    }
                });
            });
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn drop_overlay(&self, ctx: &egui::Context) {
        if ctx.input(|i| i.raw.hovered_files.is_empty()) {
            return;
        }
        let p = self.pal;
        let rect = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::unique("drop")));
        painter.rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(150));
        let inner = rect.shrink(28.0);
        painter.rect_stroke(inner, CornerRadius::same(20), Stroke::new(2.0, p.accent), egui::StrokeKind::Inside);
        painter.text(inner.center(), Align2::CENTER_CENTER, "Solte para adicionar", semibold(24.0), Color32::WHITE);
    }

    // ------------------------------------------------------------ diálogos

    fn add_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.add.take() else { return };
        let p = self.pal;
        let mut close = false;
        let mut next_stage: Option<AddStage> = None;
        let mut start: Option<(Resolved, PathBuf, bool)> = None;
        let mut resolve: Option<Source> = None;

        let modal = egui::Modal::new(Id::unique("adicionar"))
            .frame(dialog_frame(&p))
            .show(ctx, |ui| {
                ui.set_width(540.0);
                match &mut dlg.stage {
                    AddStage::Input { text, error, focus } => {
                        ui.label(RichText::new("Adicionar torrent").font(semibold(19.0)));
                        ui.label(RichText::new("Cole um link magnet, um link para um .torrent, ou escolha o arquivo.").color(p.muted));
                        ui.add_space(6.0);
                        let te = ui.add(
                            TextEdit::singleline(text)
                                .hint_text("magnet:?xt=urn:btih:…")
                                .desired_width(f32::INFINITY)
                                .margin(Margin::symmetric(10, 8)),
                        );
                        if *focus {
                            te.request_focus();
                            *focus = false;
                        }
                        if te.changed() {
                            *error = None;
                        }
                        let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if let Some(e) = error {
                            ui.label(RichText::new(e.as_str()).size(12.5).color(p.danger));
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if widgets::secondary(ui, "📄  Escolher arquivo .torrent…").clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Arquivo torrent", &["torrent"])
                                    .set_title("Escolher arquivo .torrent")
                                    .pick_file()
                            {
                                resolve = Some(Source::File(path));
                            }
                        });
                        ui.add_space(10.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let go = widgets::primary(ui, &p, "Continuar").clicked() || enter;
                            if widgets::secondary(ui, "Cancelar").clicked() {
                                close = true;
                            }
                            if go {
                                match Source::parse(text) {
                                    Some(src) => resolve = Some(src),
                                    None if text.trim().is_empty() => *error = Some("Cole um link primeiro.".into()),
                                    None => *error = Some("Isso não parece um link magnet nem um .torrent.".into()),
                                }
                            }
                        });
                    }
                    AddStage::Resolving { label, .. } => {
                        ui.label(RichText::new("Buscando informações").font(semibold(19.0)));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().size(18.0).color(p.accent));
                            ui.add(egui::Label::new(RichText::new(label.as_str()).font(semibold(14.0))).truncate());
                        });
                        ui.label(
                            RichText::new("Com links magnet, o Maré pergunta aos outros computadores quais arquivos existem. Pode levar alguns segundos.")
                                .size(12.5)
                                .color(p.muted),
                        );
                        ui.add_space(10.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::secondary(ui, "Cancelar").clicked() {
                                close = true;
                            }
                        });
                    }
                    AddStage::Files(r) => {
                        if let Some(s) = files_stage(ui, &p, r, &mut dlg.folder, &mut dlg.paused, &mut dlg.error) {
                            match s {
                                FilesAction::Cancel => close = true,
                                FilesAction::Start => {
                                    start = Some(((**r).clone(), dlg.folder.clone(), dlg.paused));
                                }
                            }
                        }
                    }
                    AddStage::Failed { message, source } => {
                        ui.label(RichText::new("Não deu certo").font(semibold(19.0)).color(p.danger));
                        ui.add_space(4.0);
                        ui.add(egui::Label::new(RichText::new(message.as_str()).color(p.text)).wrap());
                        ui.add_space(10.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if let Some(src) = source.clone()
                                && widgets::primary(ui, &p, "Tentar de novo").clicked()
                            {
                                resolve = Some(src);
                            }
                            if widgets::secondary(ui, "Fechar").clicked() {
                                close = true;
                            }
                        });
                    }
                    AddStage::Already(name) => {
                        ui.label(RichText::new("Já está na lista").font(semibold(19.0)));
                        ui.add(egui::Label::new(RichText::new(format!("“{name}” já está nos seus downloads.")).color(p.muted)).wrap());
                        ui.add_space(10.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::primary(ui, &p, "Ok").clicked() {
                                close = true;
                            }
                        });
                    }
                }
            });

        if modal.should_close() {
            close = true;
        }
        if let Some(src) = resolve {
            let req = self.next_req;
            self.next_req += 1;
            self.engine.resolve(req, src.clone());
            next_stage = Some(AddStage::Resolving { req, label: src.label(), source: src });
        }
        if let Some((r, folder, paused)) = start {
            if folder != self.settings.pasta_downloads {
                self.settings.pasta_downloads = folder.clone();
                self.save_soon();
            }
            self.engine.add(r, folder, paused);
            close = true;
        }
        if let Some(s) = next_stage {
            dlg.stage = s;
        }
        self.add = Some(dlg);
        if close {
            self.close_add();
        }
    }

    fn delete_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.delete.take() else { return };
        let p = self.pal;
        let mut done = false;
        let modal = egui::Modal::new(Id::unique("remover")).frame(dialog_frame(&p)).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(RichText::new("Remover da lista?").font(semibold(19.0)));
            ui.add(egui::Label::new(RichText::new(&dlg.name).color(p.muted)).truncate());
            ui.add_space(6.0);
            ui.checkbox(&mut dlg.delete_files, "Apagar também os arquivos baixados");
            if dlg.delete_files {
                ui.label(RichText::new("Os arquivos vão ser apagados do disco. Isso não dá para desfazer.").size(12.5).color(p.danger));
            } else {
                ui.label(RichText::new("Os arquivos continuam na pasta.").size(12.5).color(p.muted));
            }
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::danger(ui, &p, "Remover").clicked() {
                    self.engine.delete(dlg.id, dlg.delete_files);
                    done = true;
                }
                if widgets::secondary(ui, "Cancelar").clicked() {
                    done = true;
                }
            });
        });
        if !done && !modal.should_close() {
            self.delete = Some(dlg);
        }
    }

    fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let p = self.pal;
        let mut changed = false;
        let mut close = false;
        let modal = egui::Modal::new(Id::unique("config")).frame(dialog_frame(&p)).show(ctx, |ui| {
            ui.set_width(480.0);
            ui.label(RichText::new("Configurações").font(semibold(19.0)));
            ui.add_space(8.0);

            widgets::section(ui, &p, "PASTA PADRÃO DOS DOWNLOADS");
            if folder_picker(ui, &p, &mut self.settings.pasta_downloads) {
                changed = true;
            }

            ui.add_space(8.0);
            widgets::section(ui, &p, "DEPOIS DE BAIXAR");
            changed |= ui.checkbox(&mut self.settings.semear_depois, "Continuar compartilhando (semear)").changed();
            ui.label(RichText::new("Ajuda outras pessoas a baixarem o mesmo arquivo. Desligado, o download para ao terminar.").size(12.0).color(p.muted));
            changed |= ui.checkbox(&mut self.settings.notificar, "Avisar no Windows quando um download terminar").changed();

            ui.add_space(8.0);
            widgets::section(ui, &p, "APARÊNCIA");
            ui.horizontal(|ui| {
                for (t, label) in [(Tema::Escuro, "Escuro"), (Tema::Claro, "Claro"), (Tema::Sistema, "Igual ao Windows")] {
                    changed |= ui.selectable_value(&mut self.settings.tema, t, label).changed();
                }
            });

            ui.add_space(8.0);
            widgets::section(ui, &p, "REDE");
            ui.horizontal(|ui| {
                ui.label("Porta");
                changed |= ui.add(egui::DragValue::new(&mut self.settings.porta).range(1024..=65535)).changed();
                ui.label(RichText::new("vale quando o Maré abrir de novo").size(12.0).color(p.muted));
            });

            ui.add_space(14.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::primary(ui, &p, "Pronto").clicked() {
                    close = true;
                }
            });
        });
        if changed {
            self.save_soon();
        }
        if close || modal.should_close() {
            self.show_settings = false;
        }
    }
}

impl eframe::App for MareApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.update_theme(&ctx);
        self.handle_events();
        self.refresh();
        self.handle_input(&ctx);
        if self.add.is_none()
            && let Some(next) = self.queue.pop_front()
        {
            self.open_add(Some(next));
        }

        self.top_bar(ui);
        self.status_bar(ui);
        self.side_bar(ui);
        self.central(ui);

        self.add_dialog(&ctx);
        self.delete_dialog(&ctx);
        self.settings_dialog(&ctx);
        self.toasts(&ctx);
        self.drop_overlay(&ctx);

        if let Some(at) = self.save_at
            && Instant::now() >= at
        {
            self.settings.save();
            self.save_at = None;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.settings.save();
        self.engine.shutdown();
    }
}

// ---------------------------------------------------------------- auxiliares

enum FilesAction {
    Cancel,
    Start,
}

fn files_stage(
    ui: &mut Ui,
    p: &Palette,
    r: &mut Resolved,
    folder: &mut PathBuf,
    paused: &mut bool,
    error: &mut Option<String>,
) -> Option<FilesAction> {
    let mut action = None;
    let total: u64 = r.files.iter().map(|f| f.len).sum();
    let chosen: u64 = r.files.iter().filter(|f| f.selected).map(|f| f.len).sum();
    let n_sel = r.files.iter().filter(|f| f.selected).count();

    ui.horizontal(|ui| {
        ui.label(RichText::new(icon_for(&r.name)).size(20.0).color(p.accent));
        ui.add(egui::Label::new(RichText::new(&r.name).font(semibold(18.0))).truncate());
    });
    let plural = if r.files.len() == 1 { "arquivo" } else { "arquivos" };
    ui.label(
        RichText::new(format!(
            "{} {plural} · {} selecionados de {}",
            r.files.len(),
            util::bytes(chosen),
            util::bytes(total)
        ))
        .color(p.muted),
    );

    if r.files.len() > 1 {
        ui.horizontal(|ui| {
            if ui.small_button("Marcar todos").clicked() {
                r.files.iter_mut().for_each(|f| f.selected = true);
                *error = None;
            }
            if ui.small_button("Desmarcar todos").clicked() {
                r.files.iter_mut().for_each(|f| f.selected = false);
            }
        });
    }

    Frame::new()
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ScrollArea::vertical().max_height(250.0).auto_shrink([false, true]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for f in r.files.iter_mut() {
                    egui::Sides::new().truncate().show(
                        ui,
                        |ui| {
                            if ui.checkbox(&mut f.selected, "").changed() {
                                *error = None;
                            }
                            ui.label(RichText::new(icon_for(&f.path)).color(p.muted));
                            ui.add(egui::Label::new(&f.path).truncate()).on_hover_text(&f.path);
                        },
                        |ui| {
                            ui.label(RichText::new(util::bytes(f.len)).size(12.5).color(p.faint));
                        },
                    );
                }
            });
        });

    ui.add_space(6.0);
    widgets::section(ui, p, "SALVAR EM");
    folder_picker(ui, p, folder);
    if let Some(sub) = &r.subfolder {
        ui.label(RichText::new(format!("Os arquivos vão para a subpasta “{sub}”.")).size(12.0).color(p.muted));
    }
    ui.checkbox(paused, "Adicionar pausado");
    if let Some(e) = error {
        ui.label(RichText::new(e.as_str()).size(12.5).color(p.danger));
    }

    ui.add_space(10.0);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if widgets::primary(ui, p, "⬇  Baixar").clicked() {
            if n_sel == 0 {
                *error = Some("Escolha pelo menos um arquivo.".into());
            } else {
                action = Some(FilesAction::Start);
            }
        }
        if widgets::secondary(ui, "Cancelar").clicked() {
            action = Some(FilesAction::Cancel);
        }
    });
    action
}

/// Campo de pasta com botão "Alterar…". Devolve true se mudou.
fn folder_picker(ui: &mut Ui, p: &Palette, folder: &mut PathBuf) -> bool {
    let mut picked = None;
    let shown = folder.display().to_string();
    let start_dir = folder.clone();
    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Botão à direita primeiro; o caminho ocupa o resto e é cortado com "…".
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Alterar…").clicked() {
                    picked = rfd::FileDialog::new()
                        .set_title("Escolher pasta")
                        .set_directory(&start_dir)
                        .pick_folder();
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.label(RichText::new("📁").color(p.muted));
                    ui.add(egui::Label::new(shown.as_str()).truncate()).on_hover_text(shown.as_str());
                });
            });
        });
    let changed = picked.is_some();
    if let Some(f) = picked {
        *folder = f;
    }
    changed
}

fn limit_slider(ui: &mut Ui, p: &Palette, label: &str, kbps: &mut u32) -> bool {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(label).size(12.5).color(p.muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            let txt = if *kbps == 0 { "sem limite".to_string() } else { util::speed(*kbps as f64 * 1024.0) };
            ui.label(RichText::new(txt).size(12.5).color(if *kbps == 0 { p.faint } else { p.text }));
        });
    });
    // Escala por passos fixos: mais fácil de acertar do que um controle contínuo.
    const STEPS: [u32; 15] = [0, 64, 128, 256, 512, 1024, 2048, 3072, 5120, 8192, 10240, 20480, 30720, 51200, 102400];
    let mut idx = STEPS.iter().position(|s| *s >= *kbps).unwrap_or(STEPS.len() - 1);
    let resp = ui
        .horizontal(|ui| {
            ui.add_space(8.0);
            ui.spacing_mut().slider_width = ui.available_width() - 16.0;
            ui.add(egui::Slider::new(&mut idx, 0..=STEPS.len() - 1).show_value(false))
        })
        .inner;
    if resp.changed() {
        *kbps = STEPS[idx];
        return true;
    }
    false
}

fn dialog_frame(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(22))
        .shadow(egui::Shadow {
            offset: [0, 10],
            blur: 32,
            spread: 0,
            color: Color32::from_black_alpha(if p.dark { 160 } else { 50 }),
        })
}

fn pares(n: u32) -> String {
    if n == 1 { "1 par".into() } else { format!("{n} pares") }
}

fn status_label(s: Status) -> &'static str {
    match s {
        Status::Checking => "Verificando",
        Status::Downloading => "Baixando",
        Status::Seeding => "Semeando",
        Status::Done => "Concluído",
        Status::Paused => "Pausado",
        Status::Error => "Erro",
    }
}

fn icon_for(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "mkv" | "mp4" | "avi" | "mov" | "webm" | "m4v" | "wmv" => "🎬",
        "mp3" | "flac" | "ogg" | "wav" | "m4a" | "opus" | "aac" => "🎵",
        "iso" | "img" | "dmg" => "💿",
        "zip" | "rar" | "7z" | "gz" | "xz" | "tar" | "zst" => "📦",
        "jpg" | "jpeg" | "png" | "gif" | "webp" => "🖼",
        "pdf" | "epub" | "txt" | "srt" | "nfo" | "md" => "📄",
        "exe" | "msi" => "⚙",
        _ => "📁",
    }
}

fn open_in_explorer(path: &std::path::Path, folder: &std::path::Path) {
    if path.exists() && path != folder {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("explorer")
                .raw_arg(format!("/select,\"{}\"", path.display()))
                .spawn();
        }
        #[cfg(not(windows))]
        let _ = std::process::Command::new("xdg-open").arg(folder).spawn();
    } else if folder.exists() {
        let _ = std::process::Command::new("explorer").arg(folder).spawn();
    } else if let Some(parent) = folder.parent() {
        let _ = std::process::Command::new("explorer").arg(parent).spawn();
    }
}

/// Pontos do contorno de um retângulo arredondado (para a borda tracejada).
fn rounded_points(rect: egui::Rect, r: f32) -> Vec<egui::Pos2> {
    use std::f32::consts::FRAC_PI_2;
    let corners = [
        (rect.right() - r, rect.top() + r, -FRAC_PI_2),
        (rect.right() - r, rect.bottom() - r, 0.0),
        (rect.left() + r, rect.bottom() - r, FRAC_PI_2),
        (rect.left() + r, rect.top() + r, 2.0 * FRAC_PI_2),
    ];
    let mut pts = Vec::new();
    for (cx, cy, start) in corners {
        for k in 0..=8 {
            let a = start + FRAC_PI_2 * k as f32 / 8.0;
            pts.push(egui::pos2(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    pts.push(pts[0]);
    pts
}
