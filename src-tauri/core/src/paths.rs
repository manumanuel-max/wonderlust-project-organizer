//! Utilidades de rutas: compatibilidad entre sistemas, claves de comparación,
//! detección de rutas anidadas y renombrado sin sobrescritura.

use std::io;
use std::path::{Component, Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Caracteres prohibidos en nombres de archivo de Windows (y `/` en todos).
pub const WINDOWS_FORBIDDEN: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Longitud máxima de ruta clásica de Windows (MAX_PATH).
pub const WINDOWS_MAX_PATH: usize = 260;

/// Problema de compatibilidad de un componente de nombre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameIssue {
    ForbiddenChar(char),
    ControlChar,
    TrailingDotOrSpace,
    ReservedName,
    Empty,
}

impl NameIssue {
    pub fn describe(&self) -> String {
        match self {
            NameIssue::ForbiddenChar(c) => format!("contiene el carácter «{c}», no válido en Windows"),
            NameIssue::ControlChar => "contiene caracteres de control".into(),
            NameIssue::TrailingDotOrSpace => "termina en punto o espacio (no válido en Windows)".into(),
            NameIssue::ReservedName => "es un nombre reservado de Windows".into(),
            NameIssue::Empty => "está vacío".into(),
        }
    }
}

/// Comprueba si un nombre de archivo/carpeta es válido en Windows y macOS.
pub fn name_issues(name: &str) -> Vec<NameIssue> {
    let mut out = Vec::new();
    if name.is_empty() {
        out.push(NameIssue::Empty);
        return out;
    }
    for c in name.chars() {
        if WINDOWS_FORBIDDEN.contains(&c) {
            if !out.contains(&NameIssue::ForbiddenChar(c)) {
                out.push(NameIssue::ForbiddenChar(c));
            }
        } else if (c as u32) < 32 && !out.contains(&NameIssue::ControlChar) {
            out.push(NameIssue::ControlChar);
        }
    }
    if name.ends_with('.') || name.ends_with(' ') {
        out.push(NameIssue::TrailingDotOrSpace);
    }
    let stem = name.split('.').next().unwrap_or("").trim().to_uppercase();
    if WINDOWS_RESERVED.contains(&stem.as_str()) {
        out.push(NameIssue::ReservedName);
    }
    out
}

/// Sustituye caracteres incompatibles por `_` (solo se usa para nombres de
/// copias o carpetas nuevas, nunca para renombrar originales).
pub fn sanitize_component(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| {
            if WINDOWS_FORBIDDEN.contains(&c) || (c as u32) < 32 {
                '_'
            } else {
                c
            }
        })
        .collect();
    while s.ends_with('.') || s.ends_with(' ') {
        s.pop();
    }
    if s.is_empty() {
        s.push('_');
    }
    let stem = s.split('.').next().unwrap_or("").trim().to_uppercase();
    if WINDOWS_RESERVED.contains(&stem.as_str()) {
        s.insert(0, '_');
    }
    s
}

/// Clave de comparación de nombres independiente de NFC/NFD y de mayúsculas
/// (macOS y Windows suelen ser insensibles a mayúsculas).
pub fn compare_key(s: &str) -> String {
    s.nfc().collect::<String>().to_lowercase()
}

/// Une una ruta relativa con separadores `/` a una raíz, componente a componente.
pub fn join_rel(root: &Path, rel: &str) -> PathBuf {
    let mut p = root.to_path_buf();
    for part in rel.split('/').filter(|s| !s.is_empty()) {
        p.push(part);
    }
    p
}

/// Convierte una ruta relativa del sistema a texto con `/`.
pub fn rel_to_string(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Normaliza una ruta absoluta sin tocar el disco (resuelve `.` y `..`).
pub fn lexical_normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Ruta canónica si existe; si no, canónica del ancestro existente más cercano
/// más el resto de componentes.
pub fn best_effort_canonical(p: &Path) -> PathBuf {
    let p = lexical_normalize(p);
    let mut existing = p.clone();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(c) = std::fs::canonicalize(&existing) {
            let mut out = c;
            for t in tail.iter().rev() {
                out.push(t);
            }
            return out;
        }
        match (existing.file_name().map(|s| s.to_os_string()), existing.parent()) {
            (Some(name), Some(parent)) => {
                tail.push(name);
                existing = parent.to_path_buf();
            }
            _ => return p,
        }
    }
}

/// `true` si `child` es igual a `parent` o está dentro de él.
pub fn is_within(child: &Path, parent: &Path) -> bool {
    let c = best_effort_canonical(child);
    let p = best_effort_canonical(parent);
    let ck: Vec<String> = c.components().map(|x| compare_key(&x.as_os_str().to_string_lossy())).collect();
    let pk: Vec<String> = p.components().map(|x| compare_key(&x.as_os_str().to_string_lossy())).collect();
    ck.len() >= pk.len() && ck[..pk.len()] == pk[..]
}

/// En Windows añade el prefijo `\\?\` para admitir rutas largas.
pub fn long_path(p: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = p.as_os_str().to_string_lossy();
        if p.is_absolute() && !s.starts_with(r"\\?\") {
            if let Some(rest) = s.strip_prefix(r"\\") {
                return PathBuf::from(format!(r"\\?\UNC\{rest}"));
            }
            return PathBuf::from(format!(r"\\?\{s}"));
        }
        p.to_path_buf()
    }
    #[cfg(not(windows))]
    {
        p.to_path_buf()
    }
}

/// Añade un sufijo antes de la extensión: `clip.mov` → `clip_copia1.mov`.
pub fn with_suffix(name: &str, suffix: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 => format!("{}{}{}", &name[..i], suffix, &name[i..]),
        _ => format!("{name}{suffix}"),
    }
}

/// Renombra `from` a `to` **sin sobrescribir nunca** un archivo existente.
/// Devuelve `ErrorKind::AlreadyExists` si el destino existe.
pub fn rename_no_clobber(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let f = CString::new(from.as_os_str().as_bytes())?;
        let t = CString::new(to.as_os_str().as_bytes())?;
        // SAFETY: punteros válidos a cadenas terminadas en NUL.
        let r = unsafe {
            libc::renameat2(libc::AT_FDCWD, f.as_ptr(), libc::AT_FDCWD, t.as_ptr(), libc::RENAME_NOREPLACE)
        };
        if r == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EINVAL) | Some(libc::ENOSYS) | Some(libc::ENOTSUP) => fallback_rename(from, to),
            _ => Err(err),
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        extern "C" {
            fn renamex_np(from: *const libc::c_char, to: *const libc::c_char, flags: libc::c_uint) -> libc::c_int;
        }
        const RENAME_EXCL: libc::c_uint = 0x0000_0004;
        let f = CString::new(from.as_os_str().as_bytes())?;
        let t = CString::new(to.as_os_str().as_bytes())?;
        // SAFETY: punteros válidos a cadenas terminadas en NUL.
        let r = unsafe { renamex_np(f.as_ptr(), t.as_ptr(), RENAME_EXCL) };
        if r == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EINVAL) | Some(libc::ENOTSUP) => fallback_rename(from, to),
            _ => Err(err),
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
        let wide = |p: &Path| -> Vec<u16> {
            long_path(p).as_os_str().encode_wide().chain(std::iter::once(0)).collect()
        };
        let f = wide(from);
        let t = wide(to);
        // Sin MOVEFILE_REPLACE_EXISTING: falla si el destino existe.
        // SAFETY: cadenas UTF-16 terminadas en NUL.
        let ok = unsafe { MoveFileExW(f.as_ptr(), t.as_ptr(), MOVEFILE_WRITE_THROUGH) };
        if ok != 0 {
            Ok(())
        } else {
            let err = io::Error::last_os_error();
            // ERROR_FILE_EXISTS (80) / ERROR_ALREADY_EXISTS (183)
            match err.raw_os_error() {
                Some(80) | Some(183) => Err(io::Error::new(io::ErrorKind::AlreadyExists, "el destino ya existe")),
                _ => Err(err),
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        fallback_rename(from, to)
    }
}

#[allow(dead_code)]
fn fallback_rename(from: &Path, to: &Path) -> io::Result<()> {
    // Sistemas de archivos sin renombrado exclusivo (algunos SMB/exFAT):
    // primero intentar enlace duro (atómico y exclusivo); si no se admite,
    // comprobar existencia y renombrar.
    match std::fs::hard_link(from, to) {
        Ok(()) => {
            let _ = std::fs::remove_file(from);
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
        Err(_) => {
            if std::fs::symlink_metadata(to).is_ok() {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, "el destino ya existe"));
            }
            std::fs::rename(from, to)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_windows_incompatible_names() {
        assert!(name_issues("clip final.mov").is_empty());
        assert!(name_issues("Campaña Navidad ñ á é.mov").is_empty());
        assert!(name_issues("a:b.mov").contains(&NameIssue::ForbiddenChar(':')));
        assert!(name_issues("what?.wav").contains(&NameIssue::ForbiddenChar('?')));
        assert!(name_issues("con.txt").contains(&NameIssue::ReservedName));
        assert!(name_issues("dots...").contains(&NameIssue::TrailingDotOrSpace));
    }

    #[test]
    fn sanitizes() {
        assert_eq!(sanitize_component("a:b?.mov"), "a_b_.mov");
        assert_eq!(sanitize_component("NUL"), "_NUL");
        assert_eq!(sanitize_component("end. "), "end");
    }

    #[test]
    fn compare_key_unifies_nfc_nfd_and_case() {
        let nfc = "Campaña";
        let nfd = "Campan\u{0303}a";
        assert_ne!(nfc, nfd);
        assert_eq!(compare_key(nfc), compare_key(nfd));
        assert_eq!(compare_key("CLIP.MOV"), compare_key("clip.mov"));
    }

    #[test]
    fn suffix() {
        assert_eq!(with_suffix("clip.mov", "_copia1"), "clip_copia1.mov");
        assert_eq!(with_suffix("README", "_1"), "README_1");
        assert_eq!(with_suffix(".hidden", "_1"), ".hidden_1");
    }

    #[test]
    fn within() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a");
        std::fs::create_dir(&a).unwrap();
        assert!(is_within(&a.join("b/c"), &a));
        assert!(is_within(&a, &a));
        assert!(!is_within(d.path(), &a));
        assert!(is_within(&a.join("x/../y"), &a));
        assert!(!is_within(&d.path().join("ab"), &a));
    }

    #[test]
    fn rename_never_clobbers() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a.tmp");
        let b = d.path().join("b.bin");
        std::fs::write(&a, b"new").unwrap();
        std::fs::write(&b, b"old").unwrap();
        let err = rename_no_clobber(&a, &b).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&b).unwrap(), b"old");
        assert!(a.exists());
        let c = d.path().join("c.bin");
        rename_no_clobber(&a, &c).unwrap();
        assert_eq!(std::fs::read(&c).unwrap(), b"new");
        assert!(!a.exists());
    }

    #[test]
    fn fallback_rename_never_clobbers() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a.tmp");
        let b = d.path().join("b.bin");
        std::fs::write(&a, b"new").unwrap();
        std::fs::write(&b, b"old").unwrap();
        assert!(fallback_rename(&a, &b).is_err());
        assert_eq!(std::fs::read(&b).unwrap(), b"old");
    }
}
