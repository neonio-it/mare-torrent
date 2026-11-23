//! Preferências do usuário, salvas em %APPDATA%\Mare\config.json.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Tema {
    Escuro,
    Claro,
    Sistema,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Settings {
    pub pasta_downloads: PathBuf,
    /// 0 = sem limite
    pub limite_download_kbps: u32,
    /// 0 = sem limite
    pub limite_upload_kbps: u32,
    pub semear_depois: bool,
    pub notificar: bool,
    pub tema: Tema,
    pub porta: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pasta_downloads: dirs::download_dir()
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| PathBuf::from(".")),
            limite_download_kbps: 0,
            limite_upload_kbps: 0,
            semear_depois: true,
            notificar: true,
            tema: Tema::Escuro,
            porta: 6891,
        }
    }
}

/// Pasta de dados do app (configuração e sessão do librqbit).
/// `MARE_DATA_DIR` troca a pasta (modo portátil, ou para testar sem mexer na instalação).
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MARE_DATA_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Mare")
}

fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read(config_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            // Escreve num arquivo temporário e troca, para não corromper se o app fechar no meio.
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
}
