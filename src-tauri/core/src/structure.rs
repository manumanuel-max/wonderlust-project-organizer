//! Creación de la estructura de cinco carpetas.

use crate::error::{CoreError, CoreResult};
use crate::paths::{long_path, name_issues};
use crate::Category;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StructureResult {
    pub root: PathBuf,
    /// Carpetas creadas en esta operación.
    pub created: Vec<String>,
    /// Carpetas que ya existían y no se tocaron.
    pub existing: Vec<String>,
}

/// Estado de una posible carpeta de proyecto antes de crearla.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StructureCheck {
    pub root: PathBuf,
    pub root_exists: bool,
    pub parent_exists: bool,
    pub missing_folders: Vec<String>,
    pub name_problems: Vec<String>,
}

pub fn check_structure(parent: &Path, name: &str) -> StructureCheck {
    let root = parent.join(name);
    let missing_folders = Category::ALL
        .iter()
        .map(|c| c.folder_name().to_string())
        .filter(|f| !root.join(f).is_dir())
        .collect();
    StructureCheck {
        root_exists: root.exists(),
        parent_exists: parent.is_dir(),
        missing_folders,
        name_problems: name_issues(name).iter().map(|i| i.describe()).collect(),
        root,
    }
}

/// Crea `parent/name` con las cinco carpetas.
///
/// * Si la raíz no existe, se crea completa.
/// * Si existe y `complete_existing` es `false`, devuelve `AlreadyExists`.
/// * Si existe y `complete_existing` es `true`, solo crea las carpetas que
///   falten; nunca modifica ni elimina contenido existente.
pub fn create_structure(parent: &Path, name: &str, complete_existing: bool) -> CoreResult<StructureResult> {
    if name.trim().is_empty() {
        return Err(CoreError::Invalid("El nombre del proyecto está vacío".into()));
    }
    if let Some(issue) = name_issues(name).first() {
        return Err(CoreError::Invalid(format!("El nombre del proyecto {}", issue.describe())));
    }
    if !parent.is_dir() {
        return Err(CoreError::NotFound(parent.to_path_buf()));
    }
    let root = parent.join(name);
    let mut created = Vec::new();
    let mut existing = Vec::new();
    if root.exists() {
        if !root.is_dir() {
            return Err(CoreError::AlreadyExists(root));
        }
        if !complete_existing {
            return Err(CoreError::AlreadyExists(root));
        }
    } else {
        std::fs::create_dir(long_path(&root)).map_err(|e| CoreError::io(&root, e))?;
    }
    for cat in Category::ALL {
        let dir = root.join(cat.folder_name());
        if dir.is_dir() {
            existing.push(cat.folder_name().to_string());
            continue;
        }
        std::fs::create_dir(long_path(&dir)).map_err(|e| CoreError::io(&dir, e))?;
        created.push(cat.folder_name().to_string());
    }
    Ok(StructureResult { root, created, existing })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_all_five_empty_folders() {
        let d = tempfile::tempdir().unwrap();
        let r = create_structure(d.path(), "25.11.26 BANCO POPULAR - CAMPAÑA NAVIDAD", false).unwrap();
        assert_eq!(r.created.len(), 5);
        for c in Category::ALL {
            let p = r.root.join(c.folder_name());
            assert!(p.is_dir());
            assert_eq!(std::fs::read_dir(&p).unwrap().count(), 0);
        }
    }

    #[test]
    fn refuses_existing_unless_completing_and_never_touches_content() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("P");
        std::fs::create_dir_all(root.join("2. video")).unwrap();
        std::fs::write(root.join("2. video/clip.mov"), b"x").unwrap();
        assert!(matches!(create_structure(d.path(), "P", false), Err(CoreError::AlreadyExists(_))));
        let r = create_structure(d.path(), "P", true).unwrap();
        assert_eq!(r.created.len(), 4);
        assert_eq!(r.existing, vec!["2. video".to_string()]);
        assert_eq!(std::fs::read(root.join("2. video/clip.mov")).unwrap(), b"x");
    }

    #[test]
    fn rejects_invalid_names_and_missing_parent() {
        let d = tempfile::tempdir().unwrap();
        assert!(create_structure(d.path(), "a:b", false).is_err());
        assert!(create_structure(&d.path().join("nope"), "ok", false).is_err());
    }

    #[test]
    fn check_reports_missing() {
        let d = tempfile::tempdir().unwrap();
        let c = check_structure(d.path(), "X");
        assert!(!c.root_exists);
        assert_eq!(c.missing_folders.len(), 5);
    }
}
