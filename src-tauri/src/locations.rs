//! Ubicaciones frecuentes: carpetas del usuario, volúmenes externos/red y
//! Google Drive para escritorio (que se comporta como una unidad local).

use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub label: String,
    pub path: String,
    /// `user`, `volume`, `gdrive`, `recent`, `frequent`.
    pub kind: String,
    pub free_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub available: bool,
}

fn loc(label: impl Into<String>, path: &Path, kind: &str) -> Location {
    let available = path.is_dir();
    Location {
        label: label.into(),
        path: path.display().to_string(),
        kind: kind.into(),
        free_bytes: if available { fs4::available_space(path).ok() } else { None },
        total_bytes: if available { fs4::total_space(path).ok() } else { None },
        available,
    }
}

fn volumes() -> Vec<PathBuf> {
    let mut out = Vec::new();
    #[cfg(target_os = "macos")]
    if let Ok(rd) = std::fs::read_dir("/Volumes") {
        for e in rd.flatten() {
            let p = e.path();
            // Omitir el enlace al disco de arranque.
            if std::fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
                continue;
            }
            out.push(p);
        }
    }
    #[cfg(target_os = "linux")]
    {
        let user = std::env::var("USER").unwrap_or_default();
        for base in [format!("/media/{user}"), format!("/run/media/{user}"), "/mnt".to_string()] {
            if let Ok(rd) = std::fs::read_dir(&base) {
                out.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
            }
        }
    }
    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let p = PathBuf::from(format!("{}:\\", letter as char));
        if p.exists() {
            out.push(p);
        }
    }
    out
}

fn google_drive_dirs(home: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(home.join("Library/CloudStorage")) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with("GoogleDrive") {
                out.push(e.path());
            }
        }
    }
    for name in ["Google Drive", "GoogleDrive"] {
        let p = home.join(name);
        if p.is_dir() {
            out.push(p);
        }
    }
    #[cfg(windows)]
    for v in volumes() {
        for name in ["My Drive", "Mi unidad"] {
            if v.join(name).is_dir() {
                out.push(v.join(name));
            }
        }
    }
    out
}

pub fn list(app: &AppHandle, recent: &[String], frequent: &[(String, String)]) -> Vec<Location> {
    let mut out = Vec::new();
    let p = app.path();
    if let Ok(h) = p.home_dir() {
        out.push(loc("Inicio", &h, "user"));
        for g in google_drive_dirs(&h) {
            let label = g.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            out.push(loc(format!("Google Drive · {label}"), &g, "gdrive"));
        }
    }
    if let Ok(d) = p.desktop_dir() {
        out.push(loc("Escritorio", &d, "user"));
    }
    if let Ok(d) = p.document_dir() {
        out.push(loc("Documentos", &d, "user"));
    }
    if let Ok(d) = p.video_dir() {
        out.push(loc("Videos", &d, "user"));
    }
    for v in volumes() {
        let label = v
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| v.display().to_string());
        out.push(loc(label, &v, "volume"));
    }
    for (label, path) in frequent {
        out.push(loc(label.clone(), Path::new(path), "frequent"));
    }
    for r in recent {
        let pb = PathBuf::from(r);
        let label = pb.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| r.clone());
        out.push(loc(label, &pb, "recent"));
    }
    out
}
