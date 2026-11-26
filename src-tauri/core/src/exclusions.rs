//! Exclusiones configurables. Solo afectan a la copia de destino: el origen
//! nunca se modifica.

use crate::paths::compare_key;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ExclusionSettings {
    pub ae_autosave: bool,
    pub pr_autosave: bool,
    pub media_cache: bool,
    pub ae_cache: bool,
    pub render_previews: bool,
    pub proxies: bool,
    pub temp_files: bool,
    pub hidden_files: bool,
    pub system_files: bool,
    pub empty_folders: bool,
    pub custom_extensions: Vec<String>,
    pub custom_folders: Vec<String>,
}

impl Default for ExclusionSettings {
    fn default() -> Self {
        ExclusionSettings {
            ae_autosave: true,
            pr_autosave: true,
            media_cache: true,
            ae_cache: true,
            render_previews: true,
            proxies: false,
            temp_files: true,
            hidden_files: true,
            system_files: true,
            empty_folders: true,
            custom_extensions: Vec::new(),
            custom_folders: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExclusionReason {
    AeAutosave,
    PrAutosave,
    MediaCache,
    AeCache,
    RenderPreviews,
    Proxy,
    Temp,
    Hidden,
    System,
    CustomExtension,
    CustomFolder,
    /// Excluido manualmente por el usuario en la vista previa.
    Manual,
}

impl ExclusionReason {
    pub fn label(self) -> &'static str {
        match self {
            ExclusionReason::AeAutosave => "Autosave de After Effects",
            ExclusionReason::PrAutosave => "Autosave de Premiere Pro",
            ExclusionReason::MediaCache => "Adobe Media Cache",
            ExclusionReason::AeCache => "Caché de After Effects",
            ExclusionReason::RenderPreviews => "Previsualizaciones de render",
            ExclusionReason::Proxy => "Proxy",
            ExclusionReason::Temp => "Archivo temporal",
            ExclusionReason::Hidden => "Archivo oculto",
            ExclusionReason::System => "Archivo del sistema",
            ExclusionReason::CustomExtension => "Extensión excluida",
            ExclusionReason::CustomFolder => "Carpeta excluida",
            ExclusionReason::Manual => "Excluido manualmente",
        }
    }
}

const SYSTEM_FILES: &[&str] = &[".ds_store", "thumbs.db", "ehthumbs.db", "desktop.ini", "icon\r", ".localized"];
const SYSTEM_DIRS: &[&str] = &[
    ".spotlight-v100", ".trashes", ".fseventsd", ".temporaryitems", "$recycle.bin",
    "system volume information", "__macosx", ".documentrevisions-v100",
];
const AE_AUTOSAVE_DIRS: &[&str] = &["adobe after effects auto-save", "after effects auto-save"];
const PR_AUTOSAVE_DIRS: &[&str] = &["adobe premiere pro auto-save", "premiere pro auto-save"];
const MEDIA_CACHE_DIRS: &[&str] = &[
    "media cache", "media cache files", "adobe media cache", "adobe media cache files", "peak files",
    "mediacache", "common/media cache files",
];
const MEDIA_CACHE_EXTS: &[&str] = &["cfa", "pek", "mcdb", "ims", "pkf"];
const AE_CACHE_DIRS: &[&str] = &[
    "adobe after effects disk cache", "disk cache", "ae disk cache", "cache", "caches", "cache files",
];
const PREVIEW_DIRS: &[&str] = &[
    "adobe premiere pro video previews", "adobe premiere pro audio previews",
    "adobe premiere pro preview files", "adobe after effects previews",
];
const PROXY_DIRS: &[&str] = &["proxy", "proxies", "proxy media", "_proxy", "proxys", "proxy files"];
const TEMP_EXTS: &[&str] = &["tmp", "temp", "part", "crdownload", "partial", "wlpart", "download"];

/// Clave normalizada de cada componente de carpeta.
fn dir_keys(rel_dir: &str) -> Vec<String> {
    rel_dir.split('/').filter(|s| !s.is_empty()).map(compare_key).collect()
}

fn any_dir(keys: &[String], names: &[&str]) -> bool {
    keys.iter().any(|k| names.contains(&k.as_str()))
}

/// `true` si una carpeta (por su nombre) es de proxies.
pub fn is_proxy_dir(name: &str) -> bool {
    PROXY_DIRS.contains(&compare_key(name).as_str())
}

/// Evalúa si un archivo debe excluirse. `rel` es la ruta relativa con `/`.
pub fn evaluate(settings: &ExclusionSettings, rel: &str, ext: &str, hidden: bool) -> Option<ExclusionReason> {
    let (rel_dir, name) = match rel.rfind('/') {
        Some(i) => (&rel[..i], &rel[i + 1..]),
        None => ("", rel),
    };
    let keys = dir_keys(rel_dir);
    let name_key = compare_key(name);
    let ext = ext.to_lowercase();

    let is_system = SYSTEM_FILES.contains(&name_key.as_str()) || name.starts_with("._") || any_dir(&keys, SYSTEM_DIRS);
    if is_system {
        return settings.system_files.then_some(ExclusionReason::System);
    }
    // Nuestros propios temporales de copias interrumpidas se excluyen siempre.
    if ext == "wlpart" {
        return Some(ExclusionReason::Temp);
    }
    if settings.temp_files
        && (TEMP_EXTS.contains(&ext.as_str()) || name.starts_with("~$") || name.ends_with('~'))
    {
        return Some(ExclusionReason::Temp);
    }
    if settings.ae_autosave
        && (any_dir(&keys, AE_AUTOSAVE_DIRS)
            || ((ext == "aep" || ext == "aepx") && name_key.contains(" auto-save ")))
    {
        return Some(ExclusionReason::AeAutosave);
    }
    if settings.pr_autosave && any_dir(&keys, PR_AUTOSAVE_DIRS) {
        return Some(ExclusionReason::PrAutosave);
    }
    if settings.media_cache && (any_dir(&keys, MEDIA_CACHE_DIRS) || MEDIA_CACHE_EXTS.contains(&ext.as_str())) {
        return Some(ExclusionReason::MediaCache);
    }
    if settings.render_previews && any_dir(&keys, PREVIEW_DIRS) {
        return Some(ExclusionReason::RenderPreviews);
    }
    if settings.ae_cache && any_dir(&keys, AE_CACHE_DIRS) {
        return Some(ExclusionReason::AeCache);
    }
    if settings.proxies && (any_dir(&keys, PROXY_DIRS) || name_key.contains("_proxy.")) {
        return Some(ExclusionReason::Proxy);
    }
    if !settings.custom_extensions.is_empty()
        && settings
            .custom_extensions
            .iter()
            .any(|e| crate::classify::normalize_ext(e) == ext)
    {
        return Some(ExclusionReason::CustomExtension);
    }
    if !settings.custom_folders.is_empty() {
        let custom: Vec<String> = settings.custom_folders.iter().map(|s| compare_key(s.trim())).collect();
        if keys.iter().any(|k| custom.contains(k)) {
            return Some(ExclusionReason::CustomFolder);
        }
    }
    let hidden_path = hidden || name.starts_with('.') || keys.iter().any(|k| k.starts_with('.'));
    if settings.hidden_files && hidden_path {
        return Some(ExclusionReason::Hidden);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(rel: &str) -> Option<ExclusionReason> {
        let ext = rel.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        evaluate(&ExclusionSettings::default(), rel, ext, false)
    }

    #[test]
    fn default_exclusions() {
        assert_eq!(ev("AE/Adobe After Effects Auto-Save/Proyecto auto-save 3.aep"), Some(ExclusionReason::AeAutosave));
        assert_eq!(ev("Proyecto auto-save 3.aep"), Some(ExclusionReason::AeAutosave));
        assert_eq!(ev("Adobe Premiere Pro Auto-Save/Edit-2026.prproj"), Some(ExclusionReason::PrAutosave));
        assert_eq!(ev("Media Cache Files/clip.mov 48000.cfa"), Some(ExclusionReason::MediaCache));
        assert_eq!(ev("x/clip.pek"), Some(ExclusionReason::MediaCache));
        assert_eq!(ev("Adobe Premiere Pro Video Previews/a.mov"), Some(ExclusionReason::RenderPreviews));
        assert_eq!(ev("Disk Cache/x.bin"), Some(ExclusionReason::AeCache));
        assert_eq!(ev("docs/~$informe.docx"), Some(ExclusionReason::Temp));
        assert_eq!(ev("a/.DS_Store"), Some(ExclusionReason::System));
        assert_eq!(ev("a/._clip.mov"), Some(ExclusionReason::System));
        assert_eq!(ev("Thumbs.db"), Some(ExclusionReason::System));
        assert_eq!(ev(".hidden/file.txt"), Some(ExclusionReason::Hidden));
        assert_eq!(ev("x/.clip.mov.wlpart"), Some(ExclusionReason::Temp));
    }

    #[test]
    fn normal_files_and_proxies_are_included_by_default() {
        assert_eq!(ev("Footage/A001.mov"), None);
        assert_eq!(ev("Proxies/A001_Proxy.mov"), None);
        assert_eq!(ev("Proyecto.aep"), None);
        assert_eq!(ev("Edit.prproj"), None);
    }

    #[test]
    fn toggles_and_custom_rules() {
        let s = ExclusionSettings {
            proxies: true,
            system_files: false,
            custom_extensions: vec![".XMP".into()],
            custom_folders: vec!["Viejo".into()],
            ..Default::default()
        };
        assert_eq!(evaluate(&s, "Proxies/a.mov", "mov", false), Some(ExclusionReason::Proxy));
        assert_eq!(evaluate(&s, "A/.DS_Store", "", false), None);
        assert_eq!(evaluate(&s, "foto.xmp", "xmp", false), Some(ExclusionReason::CustomExtension));
        assert_eq!(evaluate(&s, "viejo/clip.mov", "mov", false), Some(ExclusionReason::CustomFolder));
    }

    #[test]
    fn windows_hidden_attribute_is_respected() {
        let s = ExclusionSettings::default();
        assert_eq!(evaluate(&s, "a.txt", "txt", true), Some(ExclusionReason::Hidden));
        let s = ExclusionSettings { hidden_files: false, ..Default::default() };
        assert_eq!(evaluate(&s, "a.txt", "txt", true), None);
    }
}
