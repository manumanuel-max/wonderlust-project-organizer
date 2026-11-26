//! Inventario recursivo de las fuentes seleccionadas.
//!
//! Solo lee metadatos; no abre contenido ni sigue enlaces simbólicos (evita
//! bucles y copias fuera del proyecto).

use crate::error::{CoreError, CoreResult};
use crate::paths::compare_key;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub id: u32,
    pub abs: PathBuf,
    /// Ruta relativa con `/` (ver [`scan`] para la raíz que se usa).
    pub rel: String,
    pub size: u64,
    /// Segundos desde UNIX_EPOCH.
    pub mtime: i64,
    pub hidden: bool,
}

impl FileEntry {
    pub fn name(&self) -> &str {
        self.rel.rsplit('/').next().unwrap_or(&self.rel)
    }
    pub fn rel_dir(&self) -> &str {
        match self.rel.rfind('/') {
            Some(i) => &self.rel[..i],
            None => "",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntryInfo {
    pub rel: String,
    pub abs: PathBuf,
    /// Sin archivos ni subcarpetas.
    pub is_empty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanIssue {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub sources: Vec<PathBuf>,
    pub files: Vec<FileEntry>,
    pub dirs: Vec<DirEntryInfo>,
    /// Elementos que no se pudieron leer.
    pub errors: Vec<ScanIssue>,
    /// Enlaces simbólicos encontrados (no se siguen ni se copian).
    pub symlinks: Vec<PathBuf>,
    /// `true` si se seleccionó una única carpeta (las rutas son relativas a ella).
    pub single_root: bool,
}

impl Inventory {
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

#[cfg(windows)]
fn is_hidden_meta(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}
#[cfg(not(windows))]
fn is_hidden_meta(_meta: &std::fs::Metadata) -> bool {
    false
}

fn mtime_of(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Escanea las fuentes.
///
/// * Una sola carpeta: las rutas relativas parten de su interior (la carpeta es el proyecto).
/// * Varias fuentes: cada carpeta conserva su nombre como primer componente y los
///   archivos sueltos quedan en la raíz relativa.
pub fn scan(
    sources: &[PathBuf],
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, u64),
) -> CoreResult<Inventory> {
    if sources.is_empty() {
        return Err(CoreError::Invalid("No se seleccionó ningún archivo o carpeta".into()));
    }
    // Eliminar fuentes duplicadas o contenidas dentro de otra fuente.
    let mut uniq: Vec<PathBuf> = Vec::new();
    for s in sources {
        if !s.exists() {
            return Err(CoreError::NotFound(s.clone()));
        }
        if !uniq.iter().any(|u| u == s) {
            uniq.push(s.clone());
        }
    }
    let uniq: Vec<PathBuf> = uniq
        .iter()
        .filter(|s| {
            !uniq
                .iter()
                .any(|o| o != *s && o.is_dir() && crate::paths::is_within(s, o))
        })
        .cloned()
        .collect();

    let single_root = uniq.len() == 1 && uniq[0].is_dir();
    let mut inv = Inventory {
        sources: uniq.clone(),
        single_root,
        ..Default::default()
    };
    let mut used_tops: HashSet<String> = HashSet::new();
    let mut next_id: u32 = 0;
    let mut bytes: u64 = 0;

    for src in &uniq {
        let meta = std::fs::symlink_metadata(src).map_err(|e| CoreError::io(src, e))?;
        if meta.file_type().is_symlink() {
            inv.symlinks.push(src.clone());
            continue;
        }
        let base_name = src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Origen".into());
        // Prefijo relativo único para esta fuente.
        let prefix = if single_root {
            String::new()
        } else {
            let mut candidate = base_name.clone();
            let mut n = 2;
            while used_tops.contains(&compare_key(&candidate)) {
                candidate = format!("{base_name} ({n})");
                n += 1;
            }
            used_tops.insert(compare_key(&candidate));
            candidate
        };
        if meta.is_file() {
            inv.files.push(FileEntry {
                id: next_id,
                abs: src.clone(),
                rel: prefix.clone(),
                size: meta.len(),
                mtime: mtime_of(&meta),
                hidden: is_hidden_name(&base_name) || is_hidden_meta(&meta),
            });
            next_id += 1;
            bytes += meta.len();
            continue;
        }
        if !prefix.is_empty() {
            // La carpeta raíz de esta fuente se registra como directorio.
            inv.dirs.push(DirEntryInfo { rel: prefix.clone(), abs: src.clone(), is_empty: false });
        }
        let mut stack: Vec<(PathBuf, String)> = vec![(src.clone(), prefix.clone())];
        while let Some((dir, rel)) = stack.pop() {
            if cancel.load(Ordering::Relaxed) {
                return Err(CoreError::Cancelled);
            }
            let rd = match std::fs::read_dir(&dir) {
                Ok(r) => r,
                Err(e) => {
                    inv.errors.push(ScanIssue { path: dir.clone(), message: e.to_string() });
                    continue;
                }
            };
            let mut children = 0usize;
            let mut subdirs = Vec::new();
            for entry in rd {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        inv.errors.push(ScanIssue { path: dir.clone(), message: e.to_string() });
                        continue;
                    }
                };
                children += 1;
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                let child_rel = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
                let meta = match std::fs::symlink_metadata(&path) {
                    Ok(m) => m,
                    Err(e) => {
                        inv.errors.push(ScanIssue { path: path.clone(), message: e.to_string() });
                        continue;
                    }
                };
                let ft = meta.file_type();
                if ft.is_symlink() {
                    inv.symlinks.push(path);
                } else if ft.is_dir() {
                    subdirs.push((path, child_rel));
                } else if ft.is_file() {
                    bytes += meta.len();
                    inv.files.push(FileEntry {
                        id: next_id,
                        abs: path,
                        rel: child_rel,
                        size: meta.len(),
                        mtime: mtime_of(&meta),
                        hidden: is_hidden_name(&name) || is_hidden_meta(&meta),
                    });
                    next_id += 1;
                    if next_id % 500 == 0 {
                        progress(inv.files.len(), bytes);
                    }
                }
            }
            if !rel.is_empty() {
                if let Some(d) = inv.dirs.iter_mut().find(|d| d.rel == rel) {
                    d.is_empty = children == 0;
                } else {
                    inv.dirs.push(DirEntryInfo { rel: rel.clone(), abs: dir.clone(), is_empty: children == 0 });
                }
            }
            // Orden estable: procesar alfabéticamente.
            subdirs.sort_by(|a, b| b.1.cmp(&a.1));
            stack.extend(subdirs);
        }
    }
    inv.files.sort_by(|a, b| a.rel.cmp(&b.rel));
    for (i, f) in inv.files.iter_mut().enumerate() {
        f.id = i as u32;
    }
    inv.dirs.sort_by(|a, b| a.rel.cmp(&b.rel));
    progress(inv.files.len(), bytes);
    Ok(inv)
}

/// Comprueba que la ruta es legible (abre el archivo en modo lectura y lo cierra).
pub fn check_readable(path: &Path) -> Result<(), String> {
    std::fs::File::open(crate::paths::long_path(path))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path, content: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    #[test]
    fn single_folder_is_project_root() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Proyecto");
        touch(&root.join("Footage/A001.mov"), b"abc");
        touch(&root.join("musica.wav"), b"x");
        std::fs::create_dir_all(root.join("Vacia")).unwrap();
        let inv = scan(&[root.clone()], &AtomicBool::new(false), |_, _| {}).unwrap();
        assert!(inv.single_root);
        let rels: Vec<_> = inv.files.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(rels, vec!["Footage/A001.mov", "musica.wav"]);
        assert_eq!(inv.total_size(), 4);
        assert!(inv.dirs.iter().any(|d| d.rel == "Vacia" && d.is_empty));
    }

    #[test]
    fn multiple_sources_keep_names_and_disambiguate() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("a/Footage/1.mov"), b"1");
        touch(&d.path().join("b/Footage/2.mov"), b"2");
        touch(&d.path().join("suelto.wav"), b"3");
        let inv = scan(
            &[d.path().join("a/Footage"), d.path().join("b/Footage"), d.path().join("suelto.wav")],
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
        let rels: Vec<_> = inv.files.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(rels, vec!["Footage (2)/2.mov", "Footage/1.mov", "suelto.wav"]);
    }

    #[test]
    fn nested_sources_are_deduplicated() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("P/sub/x.png"), b"1");
        let inv = scan(&[d.path().join("P"), d.path().join("P/sub")], &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(inv.files.len(), 1);
        assert!(inv.single_root);
    }

    #[test]
    fn handles_accents_spaces_and_special_chars() {
        let d = tempfile::tempdir().unwrap();
        let name = "Campaña Navidad (versión #2) & más ñ.mov";
        touch(&d.path().join("P/Música ñ/").join(name), b"1");
        let inv = scan(&[d.path().join("P")], &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(inv.files[0].rel, format!("Música ñ/{name}"));
        assert_eq!(inv.files[0].name(), name);
    }

    #[test]
    fn missing_source_is_an_error() {
        let d = tempfile::tempdir().unwrap();
        assert!(scan(&[d.path().join("nope")], &AtomicBool::new(false), |_, _| {}).is_err());
    }

    #[test]
    fn cancellation() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("P/a.txt"), b"1");
        assert!(matches!(
            scan(&[d.path().join("P")], &AtomicBool::new(true), |_, _| {}),
            Err(CoreError::Cancelled)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("P/a.txt"), b"1");
        std::os::unix::fs::symlink(d.path(), d.path().join("P/loop")).unwrap();
        let inv = scan(&[d.path().join("P")], &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(inv.files.len(), 1);
        assert_eq!(inv.symlinks.len(), 1);
    }
}
