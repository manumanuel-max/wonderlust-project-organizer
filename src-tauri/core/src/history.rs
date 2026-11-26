//! Historial local de operaciones. Eliminar registros nunca elimina archivos
//! de proyectos: solo se borra la entrada (y su reporte guardado si se pide).

use crate::error::CoreResult;
use crate::settings::{load_json, save_json};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MAX_ENTRIES: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub date: String,
    pub client: String,
    pub project: String,
    /// `structure`, `organize` o `backup`.
    pub kind: String,
    pub destination: String,
    pub files: usize,
    pub size: u64,
    /// `completed`, `completedWithErrors`, `cancelled`, `aborted`, `created`, `running`.
    pub status: String,
    /// Identificador del trabajo con reporte asociado.
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

pub fn load(path: &Path) -> History {
    load_json(path)
}

pub fn upsert(path: &Path, entry: HistoryEntry) -> CoreResult<()> {
    let mut h = load(path);
    h.entries.retain(|e| e.id != entry.id);
    h.entries.insert(0, entry);
    h.entries.truncate(MAX_ENTRIES);
    save_json(path, &h)
}

pub fn delete(path: &Path, ids: &[String]) -> CoreResult<usize> {
    let mut h = load(path);
    let before = h.entries.len();
    h.entries.retain(|e| !ids.contains(&e.id));
    let removed = before - h.entries.len();
    save_json(path, &h)?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> HistoryEntry {
        HistoryEntry {
            id: id.into(),
            date: "2026-11-25T10:00:00-05:00".into(),
            client: "BANCO POPULAR".into(),
            project: "CAMPAÑA NAVIDAD".into(),
            kind: "organize".into(),
            destination: "/tmp/x".into(),
            files: 3,
            size: 10,
            status: "completed".into(),
            job_id: Some(id.into()),
        }
    }

    #[test]
    fn upsert_and_delete_never_touch_project_files() {
        let d = tempfile::tempdir().unwrap();
        let proj = d.path().join("proyecto");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join("a.mov"), b"x").unwrap();
        let p = d.path().join("history.json");
        upsert(&p, entry("1")).unwrap();
        upsert(&p, entry("2")).unwrap();
        let mut e = entry("1");
        e.status = "completedWithErrors".into();
        upsert(&p, e).unwrap();
        let h = load(&p);
        assert_eq!(h.entries.len(), 2);
        assert_eq!(h.entries[0].id, "1");
        assert_eq!(delete(&p, &["1".into()]).unwrap(), 1);
        assert_eq!(load(&p).entries.len(), 1);
        assert!(proj.join("a.mov").exists());
    }
}
