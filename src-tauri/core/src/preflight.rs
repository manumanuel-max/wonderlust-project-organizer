//! Verificación previa a la copia: nada se escribe en el destino (salvo un
//! archivo de prueba de escritura que se elimina inmediatamente) y nada se
//! modifica en el origen.

use crate::duplicates::{OpAction, OperationSet};
use crate::paths::{best_effort_canonical, is_within, join_rel, name_issues, WINDOWS_MAX_PATH};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueLevel {
    Blocking,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightIssue {
    pub code: String,
    pub level: IssueLevel,
    pub title: String,
    pub detail: String,
    pub paths: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub ok: bool,
    pub issues: Vec<PreflightIssue>,
    pub required_bytes: u64,
    pub available_bytes: Option<u64>,
    pub files_to_copy: usize,
    pub files_skipped: usize,
    pub filesystem: Option<String>,
}

/// Ancestro existente más cercano.
pub fn nearest_existing(p: &Path) -> Option<PathBuf> {
    let mut cur = Some(p);
    while let Some(c) = cur {
        if c.exists() {
            return Some(c.to_path_buf());
        }
        cur = c.parent();
    }
    None
}

/// Espacio libre en el volumen que contiene `p`.
pub fn available_space(p: &Path) -> Option<u64> {
    let base = nearest_existing(p)?;
    fs4::available_space(base).ok()
}

/// Tipo de sistema de archivos (en minúsculas) cuando se puede determinar.
pub fn filesystem_type(p: &Path) -> Option<String> {
    let base = nearest_existing(p)?;
    #[cfg(target_os = "macos")]
    {
        use std::ffi::{CStr, CString};
        use std::os::unix::ffi::OsStrExt;
        let c = CString::new(base.as_os_str().as_bytes()).ok()?;
        let mut s: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: ruta válida y estructura inicializada.
        if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
            return None;
        }
        let name = unsafe { CStr::from_ptr(s.f_fstypename.as_ptr()) };
        return Some(name.to_string_lossy().to_lowercase());
    }
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let c = CString::new(base.as_os_str().as_bytes()).ok()?;
        let mut s: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: ruta válida y estructura inicializada.
        if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
            return None;
        }
        let t = s.f_type as i64;
        let name = match t {
            0x4d44 => "msdos",
            0x2011BAB0 => "exfat",
            0x5346544e => "ntfs",
            0xEF53 => "ext4",
            0x9123683E => "btrfs",
            0x58465342 => "xfs",
            0x01021994 => "tmpfs",
            0x6969 => "nfs",
            0xFF534D42 | 0xFE534D42 | 0x517B => "smb",
            0x794c7630 => "overlay",
            _ => "otro",
        };
        return Some(name.to_string());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};
        let wide: Vec<u16> = base.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let mut vol = [0u16; 261];
        // SAFETY: búferes de tamaño correcto.
        if unsafe { GetVolumePathNameW(wide.as_ptr(), vol.as_mut_ptr(), vol.len() as u32) } == 0 {
            return None;
        }
        let mut fs_name = [0u16; 64];
        let ok = unsafe {
            GetVolumeInformationW(
                vol.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fs_name.as_mut_ptr(),
                fs_name.len() as u32,
            )
        };
        if ok == 0 {
            return None;
        }
        let len = fs_name.iter().position(|c| *c == 0).unwrap_or(0);
        return Some(String::from_utf16_lossy(&fs_name[..len]).to_lowercase());
    }
    #[allow(unreachable_code)]
    {
        let _ = base;
        None
    }
}

fn issue(code: &str, level: IssueLevel, title: String, detail: String, paths: Vec<String>) -> PreflightIssue {
    let count = paths.len();
    PreflightIssue { code: code.into(), level, title, detail, paths: paths.into_iter().take(50).collect(), count }
}

pub fn run_preflight(
    ops: &OperationSet,
    sources: &[PathBuf],
    dest_root: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize),
) -> PreflightReport {
    let mut issues = Vec::new();
    let copies: Vec<_> = ops.ops.iter().filter(|o| o.action == OpAction::Copy).collect();
    let required: u64 = copies.iter().map(|o| o.size).sum();
    let skipped = ops.ops.len() - copies.len();

    // 1. Destino.
    let parent_ok = match dest_root.parent() {
        Some(p) if p.is_dir() => true,
        _ => {
            issues.push(issue(
                "dest-missing",
                IssueLevel::Blocking,
                "La ubicación de destino no existe".into(),
                format!("No se encontró «{}». Conecta la unidad o elige otra ubicación.", dest_root.parent().unwrap_or(dest_root).display()),
                vec![],
            ));
            false
        }
    };
    if dest_root.exists() && !dest_root.is_dir() {
        issues.push(issue(
            "dest-not-dir",
            IssueLevel::Blocking,
            "El destino es un archivo".into(),
            format!("«{}» existe y no es una carpeta.", dest_root.display()),
            vec![],
        ));
    } else if dest_root.is_dir() {
        issues.push(issue(
            "dest-exists",
            IssueLevel::Info,
            "La carpeta del proyecto ya existe en el destino".into(),
            "Se añadirán los archivos sin sobrescribir nada; los conflictos se muestran en la tabla de conflictos.".into(),
            vec![],
        ));
    }

    // 2. Copia recursiva / destino dentro del origen.
    for s in sources {
        if s.is_dir() && is_within(dest_root, s) {
            issues.push(issue(
                "dest-inside-source",
                IssueLevel::Blocking,
                "El destino está dentro del origen".into(),
                format!(
                    "Copiar en «{}» crearía una copia recursiva dentro de «{}». Elige un destino fuera del origen.",
                    dest_root.display(),
                    s.display()
                ),
                vec![s.display().to_string()],
            ));
        } else if is_within(s, dest_root) {
            issues.push(issue(
                "source-inside-dest",
                IssueLevel::Warning,
                "Un origen está dentro de la carpeta de destino".into(),
                "Los originales no se modificarán, pero la copia se mezclará con ellos. Se recomienda otro destino.".into(),
                vec![s.display().to_string()],
            ));
        }
    }

    // 3. Permiso de escritura (archivo de prueba propio que se elimina).
    if parent_ok {
        let probe_dir = nearest_existing(dest_root).unwrap_or_else(|| dest_root.to_path_buf());
        let probe = probe_dir.join(format!(".wonderlust_probe_{}", uuid::Uuid::new_v4().simple()));
        let res = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .and_then(|mut f| f.write_all(b"ok"));
        let _ = std::fs::remove_file(&probe);
        if let Err(e) = res {
            issues.push(issue(
                "dest-not-writable",
                IssueLevel::Blocking,
                "Sin permiso de escritura en el destino".into(),
                format!("{}: {e}", probe_dir.display()),
                vec![],
            ));
        }
    }

    // 4. Espacio disponible (con margen del 1 % + 50 MB).
    let available = available_space(dest_root);
    if let Some(av) = available {
        let needed = required + required / 100 + 50 * 1024 * 1024;
        if needed > av {
            issues.push(issue(
                "no-space",
                IssueLevel::Blocking,
                "Espacio insuficiente en el destino".into(),
                format!(
                    "Se necesitan {} y hay {} disponibles.",
                    crate::report::human_size(needed),
                    crate::report::human_size(av)
                ),
                vec![],
            ));
        }
    }

    // 5. Sistema de archivos FAT32: límite de 4 GB por archivo.
    let fs = filesystem_type(dest_root);
    if matches!(fs.as_deref(), Some("msdos") | Some("fat32") | Some("fat")) {
        let big: Vec<String> = copies.iter().filter(|o| o.size >= 4 * 1024 * 1024 * 1024 - 1).map(|o| o.rel_src.clone()).collect();
        if !big.is_empty() {
            issues.push(issue(
                "fat32-limit",
                IssueLevel::Blocking,
                "El destino es FAT32 y hay archivos de 4 GB o más".into(),
                "FAT32 no admite archivos de 4 GB o más. Usa una unidad exFAT, NTFS o APFS.".into(),
                big,
            ));
        }
    }

    // 6. Lectura de orígenes, rutas largas y nombres incompatibles.
    let mut unreadable = Vec::new();
    let mut long_paths = Vec::new();
    let mut bad_names = Vec::new();
    let total = copies.len();
    let dest_canon = best_effort_canonical(dest_root);
    for (n, o) in copies.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if n % 250 == 0 {
            progress(n, total);
        }
        if let Err(e) = crate::scan::check_readable(&o.src) {
            unreadable.push(format!("{} — {e}", o.src.display()));
        }
        let abs = join_rel(&dest_canon, &o.dest_rel);
        let len = abs.as_os_str().len();
        if len >= WINDOWS_MAX_PATH - 12 {
            long_paths.push(format!("{} ({len} caracteres)", o.dest_rel));
        }
        for comp in o.dest_rel.split('/') {
            let probs = name_issues(comp);
            if !probs.is_empty() {
                bad_names.push(format!("{} — {}", o.dest_rel, probs[0].describe()));
                break;
            }
        }
    }
    progress(total, total);
    if !unreadable.is_empty() {
        issues.push(issue(
            "unreadable",
            IssueLevel::Warning,
            format!("{} archivo(s) del origen no se pueden leer", unreadable.len()),
            "Estos archivos fallarán al copiar y el backup no se marcará como completo. Revisa permisos o exclúyelos.".into(),
            unreadable,
        ));
    }
    if !long_paths.is_empty() {
        issues.push(issue(
            "long-paths",
            IssueLevel::Warning,
            format!("{} ruta(s) superan el límite clásico de Windows", long_paths.len()),
            "La aplicación puede copiarlas, pero el Explorador de Windows y algunas aplicaciones podrían no abrirlas. \
             Considera un destino con una ruta más corta."
                .into(),
            long_paths,
        ));
    }
    if !bad_names.is_empty() {
        issues.push(issue(
            "incompatible-names",
            IssueLevel::Warning,
            format!("{} nombre(s) incompatibles con Windows", bad_names.len()),
            "Los nombres no se modifican. Si el destino o un equipo futuro usa Windows, estos archivos podrían no copiarse \
             o no abrirse."
                .into(),
            bad_names,
        ));
    }
    if copies.is_empty() {
        issues.push(issue(
            "nothing-to-copy",
            IssueLevel::Warning,
            "No hay archivos para copiar".into(),
            "Todos los archivos están excluidos u omitidos.".into(),
            vec![],
        ));
    }

    PreflightReport {
        ok: !issues.iter().any(|i| i.level == IssueLevel::Blocking),
        issues,
        required_bytes: required,
        available_bytes: available,
        files_to_copy: copies.len(),
        files_skipped: skipped,
        filesystem: fs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duplicates::Operation;

    fn ops(src: &Path, rels: &[(&str, u64)]) -> OperationSet {
        OperationSet {
            ops: rels
                .iter()
                .enumerate()
                .map(|(i, (r, s))| Operation {
                    idx: i as u32,
                    item_id: i as u32,
                    src: src.join(r),
                    rel_src: r.to_string(),
                    dest_rel: format!("2. video/{r}"),
                    size: *s,
                    mtime: 0,
                    category: crate::Category::Video,
                    action: OpAction::Copy,
                    renamed: false,
                })
                .collect(),
            dirs: vec![],
            excluded: vec![],
        }
    }

    #[test]
    fn ok_case() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.mov"), b"1").unwrap();
        let r = run_preflight(&ops(&src, &[("a.mov", 1)]), &[src.clone()], &d.path().join("out/P"), &AtomicBool::new(false), |_, _| {});
        // out/ no existe todavía: el padre del proyecto debe existir.
        assert!(!r.ok);
        std::fs::create_dir_all(d.path().join("out")).unwrap();
        let r = run_preflight(&ops(&src, &[("a.mov", 1)]), &[src.clone()], &d.path().join("out/P"), &AtomicBool::new(false), |_, _| {});
        assert!(r.ok, "{:?}", r.issues);
        assert_eq!(r.required_bytes, 1);
        // No quedan archivos de prueba.
        assert_eq!(std::fs::read_dir(d.path().join("out")).unwrap().count(), 0);
    }

    #[test]
    fn blocks_destination_inside_source() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.mov"), b"1").unwrap();
        let r = run_preflight(&ops(&src, &[("a.mov", 1)]), &[src.clone()], &src.join("Backup"), &AtomicBool::new(false), |_, _| {});
        assert!(!r.ok);
        assert!(r.issues.iter().any(|i| i.code == "dest-inside-source"));
    }

    #[test]
    fn blocks_when_not_enough_space() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.mov"), b"1").unwrap();
        let r = run_preflight(&ops(&src, &[("a.mov", u64::MAX / 4)]), &[src.clone()], &d.path().join("P"), &AtomicBool::new(false), |_, _| {});
        assert!(r.issues.iter().any(|i| i.code == "no-space"));
        assert!(!r.ok);
    }

    #[test]
    fn reports_unreadable_and_long_paths() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let long = format!("{}.mov", "x".repeat(250));
        let r = run_preflight(&ops(&src, &[("missing.mov", 1), (&long, 1)]), &[src.clone()], &d.path().join("P"), &AtomicBool::new(false), |_, _| {});
        assert!(r.issues.iter().any(|i| i.code == "unreadable" && i.count == 2));
        assert!(r.issues.iter().any(|i| i.code == "long-paths"));
    }

    #[cfg(unix)]
    #[test]
    fn detects_read_only_destination() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return; // root ignora permisos
        }
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let ro = d.path().join("ro");
        std::fs::create_dir_all(&ro).unwrap();
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
        let r = run_preflight(&ops(&src, &[]), &[src.clone()], &ro.join("P"), &AtomicBool::new(false), |_, _| {});
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(r.issues.iter().any(|i| i.code == "dest-not-writable"));
    }
}
