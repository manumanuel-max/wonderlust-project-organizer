//! "Buscar actualizaciones": comprueba, descarga, verifica la firma y
//! prepara la instalación de una versión nueva.
//!
//! La URL del canal (`latest.json`) se fija al compilar con la variable de
//! entorno `WPO_UPDATE_URL`. Si no se definió, la función queda desactivada y
//! la interfaz lo indica. La firma se valida siempre con la clave pública de
//! `tauri.conf.json`; un paquete sin firma válida se rechaza.

use crate::state::{err, AppState};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Canal de actualizaciones definido al compilar.
pub const UPDATE_URL: Option<&str> = option_env!("WPO_UPDATE_URL");

#[derive(Default)]
pub struct UpdateState {
    pub pending: Mutex<Option<Update>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConfig {
    pub configured: bool,
    pub current_version: String,
    pub channel: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

fn channel() -> Option<&'static str> {
    UPDATE_URL.map(str::trim).filter(|u| !u.is_empty())
}

#[tauri::command]
pub fn update_config(app: AppHandle) -> UpdateConfig {
    UpdateConfig {
        configured: channel().is_some(),
        current_version: app.package_info().version.to_string(),
        channel: channel().map(String::from),
    }
}

fn updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let url = channel().ok_or("Las actualizaciones automáticas no están configuradas en esta compilación")?;
    let parsed: tauri::Url = url.parse().map_err(|_| format!("URL de actualizaciones no válida: {url}"))?;
    // En compilaciones de producción el plugin exige HTTPS.
    app.updater_builder().endpoints(vec![parsed]).map_err(err)?.build().map_err(err)
}

/// Comprueba si hay una versión nueva. `None` = ya está actualizada.
#[tauri::command]
pub async fn check_update(app: AppHandle, upd: State<'_, UpdateState>) -> Result<Option<UpdateInfo>, String> {
    let u = updater(&app)?;
    let found = u
        .check()
        .await
        .map_err(|e| format!("No se pudo comprobar si hay actualizaciones: {e}"))?;
    let info = found.as_ref().map(|f| UpdateInfo {
        version: f.version.clone(),
        current_version: f.current_version.clone(),
        date: f.date.map(|d| d.to_string()),
        notes: f.body.clone(),
    });
    *upd.pending.lock().unwrap() = found;
    Ok(info)
}

/// Descarga, verifica la firma e instala. Después la interfaz pide reiniciar.
#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, AppState>, upd: State<'_, UpdateState>) -> Result<(), String> {
    if !state.jobs.lock().unwrap().is_empty() {
        return Err("Hay una copia o movimiento en curso. Espera a que termine antes de actualizar.".into());
    }
    let update = upd
        .pending
        .lock()
        .unwrap()
        .take()
        .ok_or("Primero busca actualizaciones")?;
    let mut downloaded = 0u64;
    let app2 = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = app2.emit("update-progress", UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.to_lowercase().contains("signature") {
                format!("La actualización se rechazó porque su firma no es válida ({msg}). No se instaló nada.")
            } else {
                format!("No se pudo instalar la actualización: {msg}")
            }
        })
}

#[tauri::command]
pub fn restart_app(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    if !state.jobs.lock().unwrap().is_empty() {
        return Err("Hay una operación en curso".into());
    }
    app.restart()
}
