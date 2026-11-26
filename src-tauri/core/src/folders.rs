//! Detección funcional de subcarpetas mediante reglas locales (nombres,
//! extensiones y estructura). Sin IA externa.

use crate::classify::{extension_of, VIDEO_EXTS};
use crate::paths::compare_key;
use crate::scan::Inventory;
use crate::Category;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupKind {
    /// Tarjeta de cámara completa (DCIM, PRIVATE/M4ROOT, XDROOT, CONTENTS, .RDM...).
    CameraCard,
    /// Proyecto que ya sigue la estructura estandarizada.
    OrganizedProject,
    Proxies,
    Renders,
    Music,
    Sfx,
    Voiceover,
    Stock,
    Graphics,
    Autosave,
    Cache,
}

impl GroupKind {
    pub fn label(self) -> &'static str {
        match self {
            GroupKind::CameraCard => "Tarjeta de cámara",
            GroupKind::OrganizedProject => "Proyecto ya organizado",
            GroupKind::Proxies => "Proxies",
            GroupKind::Renders => "Renders / entregables",
            GroupKind::Music => "Música",
            GroupKind::Sfx => "Efectos de sonido",
            GroupKind::Voiceover => "Locuciones",
            GroupKind::Stock => "Material de stock",
            GroupKind::Graphics => "Recursos gráficos",
            GroupKind::Autosave => "Autosaves",
            GroupKind::Cache => "Cachés",
        }
    }

    /// Los grupos "de subárbol completo" conservan toda su estructura interna.
    pub fn keeps_whole_subtree(self) -> bool {
        matches!(self, GroupKind::CameraCard | GroupKind::OrganizedProject)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderGroup {
    /// Ruta relativa de la carpeta en el origen (clave estable).
    pub rel_dir: String,
    /// Nombre a usar en destino.
    pub name: String,
    pub kind: GroupKind,
    /// Categoría principal para tarjetas de cámara.
    pub category: Option<Category>,
    pub file_count: usize,
    pub size: u64,
    pub detail: String,
    /// `false` si el usuario decidió tratarla como una carpeta normal.
    pub enabled: bool,
}

const CARD_MARKERS: &[&str] = &[
    "dcim", "private", "xdroot", "contents", "avchd", "bpav", "m4root", "clip", "sonyclip", "cameraroll",
];
const MUSIC: &[&str] = &[
    "musica", "música", "music", "songs", "canciones", "tracks", "bso", "soundtrack", "banda sonora",
];
const SFX: &[&str] = &[
    "sfx", "fx", "efectos", "efectos de sonido", "sound effects", "soundfx", "foley", "sonidos", "sounds",
];
const VO: &[&str] = &[
    "locucion", "locución", "locuciones", "vo", "voice over", "voiceover", "voice-over", "voz", "voces",
    "narracion", "narración", "voz en off",
];
const STOCK: &[&str] = &[
    "stock", "stock footage", "footage stock", "envato", "artgrid", "artlist", "shutterstock", "pond5",
    "storyblocks", "adobe stock", "motion array", "getty", "istock",
];
const GRAPHICS: &[&str] = &[
    "graficos", "gráficos", "graphics", "gfx", "assets", "logos", "logo", "recursos", "elementos",
    "ilustraciones", "branding", "brand", "lower thirds", "titulos", "títulos", "fuentes", "fonts",
];
const AUTOSAVE: &[&str] = &["adobe after effects auto-save", "adobe premiere pro auto-save"];
const CACHE: &[&str] = &[
    "media cache", "media cache files", "adobe media cache", "adobe media cache files",
    "adobe after effects disk cache", "disk cache", "cache", "caches",
];

/// Patrones predeterminados de renders finales.
pub fn default_render_patterns() -> Vec<String> {
    [
        "FINAL", "FINALES", "EXPORT", "EXPORTS", "RENDER", "RENDERS", "ENTREGABLE", "ENTREGABLES", "MASTER",
        "DELIVERY", "DELIVERABLES", "APROBADO",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Divide un nombre en palabras (alfanuméricas) en minúsculas.
pub fn tokens(name: &str) -> Vec<String> {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

/// `true` si el nombre contiene algún patrón de render como palabra completa.
pub fn matches_render_pattern(name: &str, patterns: &[String]) -> bool {
    let toks = tokens(name);
    patterns.iter().any(|p| {
        let p = p.trim().to_lowercase();
        !p.is_empty() && toks.iter().any(|t| *t == p)
    })
}

fn last_component(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn in_list(name: &str, list: &[&str]) -> bool {
    list.contains(&compare_key(name.trim()).as_str())
}

/// Detecta grupos funcionales en el inventario.
pub fn detect_groups(inv: &Inventory, root_name: &str, render_patterns: &[String]) -> Vec<FolderGroup> {
    // Hijos directos (carpetas) de cada carpeta.
    let mut child_dirs: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut all_dirs: Vec<&str> = inv.dirs.iter().map(|d| d.rel.as_str()).collect();
    // Las carpetas intermedias pueden no estar registradas si solo hay archivos.
    for f in &inv.files {
        let mut d = f.rel_dir();
        while !d.is_empty() {
            all_dirs.push(d);
            d = match d.rfind('/') {
                Some(i) => &d[..i],
                None => "",
            };
        }
    }
    all_dirs.sort_unstable();
    all_dirs.dedup();
    for d in &all_dirs {
        let parent = match d.rfind('/') {
            Some(i) => &d[..i],
            None => "",
        };
        child_dirs.entry(parent).or_default().push(last_component(d));
    }

    // Estadísticas por subárbol.
    let mut subtree: BTreeMap<String, (usize, u64, bool, bool)> = BTreeMap::new(); // files, size, has_video, has_image
    for f in &inv.files {
        let ext = extension_of(Path::new(f.name()));
        let is_video = VIDEO_EXTS.contains(&ext.as_str());
        let is_image = crate::classify::IMAGE_EXTS.contains(&ext.as_str());
        let mut d = f.rel_dir();
        loop {
            let e = subtree.entry(d.to_string()).or_insert((0, 0, false, false));
            e.0 += 1;
            e.1 += f.size;
            e.2 |= is_video;
            e.3 |= is_image;
            if d.is_empty() {
                break;
            }
            d = match d.rfind('/') {
                Some(i) => &d[..i],
                None => "",
            };
        }
    }

    let mut out: Vec<FolderGroup> = Vec::new();
    let mut claimed: Vec<String> = Vec::new(); // subárboles completos ya asignados
    let mut candidates: Vec<&str> = vec![""];
    candidates.extend(all_dirs.iter().copied());

    for d in candidates {
        if claimed.iter().any(|c| d == c || (c.is_empty() && !d.is_empty()) || d.starts_with(&format!("{c}/"))) {
            continue;
        }
        let name = if d.is_empty() { root_name } else { last_component(d) };
        let (count, size, has_video, has_image) = subtree.get(d).copied().unwrap_or((0, 0, false, false));
        if count == 0 {
            continue;
        }
        let children: Vec<String> = child_dirs
            .get(d)
            .map(|v| v.iter().map(|c| compare_key(c)).collect())
            .unwrap_or_default();
        let mk = |kind: GroupKind, category: Option<Category>, detail: String| FolderGroup {
            rel_dir: d.to_string(),
            name: name.to_string(),
            kind,
            category,
            file_count: count,
            size,
            detail,
            enabled: true,
        };

        // 1. Proyecto ya organizado (≥ 3 de las 5 carpetas estándar).
        let std_count = child_dirs
            .get(d)
            .map(|v| v.iter().filter(|c| Category::from_folder_name(c).is_some()).count())
            .unwrap_or(0);
        if std_count >= 3 {
            out.push(mk(
                GroupKind::OrganizedProject,
                None,
                format!("Contiene {std_count} de las 5 carpetas estándar"),
            ));
            claimed.push(d.to_string());
            continue;
        }

        // 2. Tarjeta de cámara.
        let lower = compare_key(name);
        let red_mag = lower.ends_with(".rdm") || lower.ends_with(".rdc");
        let markers: Vec<&String> = children.iter().filter(|c| CARD_MARKERS.contains(&c.as_str())).collect();
        let is_card = red_mag
            || (!markers.is_empty()
                && (has_video || has_image)
                && (children.iter().any(|c| c == "dcim" || c == "private" || c == "xdroot" || c == "contents" || c == "avchd" || c == "bpav")));
        if is_card {
            let cat = if has_video { Category::Video } else { Category::Imagen };
            let detail = if red_mag {
                "Magazine/clip de RED".to_string()
            } else {
                format!("Estructura de cámara: {}", markers.iter().map(|s| s.to_uppercase()).collect::<Vec<_>>().join(", "))
            };
            out.push(mk(GroupKind::CameraCard, Some(cat), detail));
            claimed.push(d.to_string());
            continue;
        }
        if d.is_empty() {
            continue;
        }

        // 3. Carpetas funcionales por nombre.
        let kind = if in_list(name, AUTOSAVE) {
            Some(GroupKind::Autosave)
        } else if in_list(name, CACHE) {
            Some(GroupKind::Cache)
        } else if crate::exclusions::is_proxy_dir(name) {
            Some(GroupKind::Proxies)
        } else if matches_render_pattern(name, render_patterns) {
            Some(GroupKind::Renders)
        } else if in_list(name, MUSIC) {
            Some(GroupKind::Music)
        } else if in_list(name, SFX) {
            Some(GroupKind::Sfx)
        } else if in_list(name, VO) {
            Some(GroupKind::Voiceover)
        } else if in_list(name, STOCK) || tokens(name).iter().any(|t| t == "stock") {
            Some(GroupKind::Stock)
        } else if in_list(name, GRAPHICS) {
            Some(GroupKind::Graphics)
        } else {
            None
        };
        if let Some(kind) = kind {
            out.push(mk(kind, None, format!("Carpeta «{name}»")));
        }
    }
    out
}

/// Devuelve el grupo más profundo que contiene `rel_dir` (o la propia carpeta).
pub fn find_anchor<'a>(groups: &'a [FolderGroup], rel_dir: &str) -> Option<&'a FolderGroup> {
    groups
        .iter()
        .filter(|g| g.enabled)
        .filter(|g| g.rel_dir == rel_dir || g.rel_dir.is_empty() || rel_dir.starts_with(&format!("{}/", g.rel_dir)))
        .max_by_key(|g| if g.rel_dir.is_empty() { 0 } else { g.rel_dir.len() + 1 })
}

/// Ruta de `rel_dir` relativa al grupo (vacía si es la propia carpeta del grupo).
pub fn rel_within(group_dir: &str, rel_dir: &str) -> String {
    if group_dir.is_empty() {
        return rel_dir.to_string();
    }
    if rel_dir == group_dir {
        return String::new();
    }
    rel_dir
        .strip_prefix(&format!("{group_dir}/"))
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{DirEntryInfo, FileEntry};

    fn inv(paths: &[&str]) -> Inventory {
        let files = paths
            .iter()
            .enumerate()
            .map(|(i, p)| FileEntry {
                id: i as u32,
                abs: format!("/src/{p}").into(),
                rel: p.to_string(),
                size: 10,
                mtime: 0,
                hidden: false,
            })
            .collect();
        Inventory { files, dirs: Vec::<DirEntryInfo>::new(), single_root: true, ..Default::default() }
    }

    fn kinds(groups: &[FolderGroup]) -> Vec<(String, GroupKind)> {
        groups.iter().map(|g| (g.rel_dir.clone(), g.kind)).collect()
    }

    #[test]
    fn detects_camera_cards() {
        let i = inv(&[
            "Rodaje/Dia1/CardA/PRIVATE/M4ROOT/CLIP/C0001.MP4",
            "Rodaje/Dia1/CardA/PRIVATE/M4ROOT/CLIP/C0001M01.XML",
            "Rodaje/Dia1/CardA/DCIM/100MSDCF/x.jpg",
            "Rodaje/RED/A001_C001.RDM/A001_C001_0101.RDC/A001_C001_0101_001.R3D",
        ]);
        let g = detect_groups(&i, "P", &default_render_patterns());
        let k = kinds(&g);
        assert!(k.contains(&("Rodaje/Dia1/CardA".into(), GroupKind::CameraCard)));
        assert!(k.contains(&("Rodaje/RED/A001_C001.RDM".into(), GroupKind::CameraCard)));
        // El .RDC interno no se detecta por separado: pertenece al .RDM.
        assert!(!k.iter().any(|(d, _)| d.ends_with(".RDC")));
        let card = g.iter().find(|x| x.rel_dir == "Rodaje/Dia1/CardA").unwrap();
        assert_eq!(card.category, Some(Category::Video));
    }

    #[test]
    fn detects_functional_folders() {
        let i = inv(&[
            "Audio/Música/tema.wav",
            "Audio/SFX/whoosh.wav",
            "Audio/Locución/vo_toma1.wav",
            "Renders/Spot_30s.mov",
            "Proxies/A001_Proxy.mov",
            "Stock/Envato/clip.mp4",
            "Graficos/logo.ai",
        ]);
        let g = detect_groups(&i, "P", &default_render_patterns());
        let k = kinds(&g);
        assert!(k.contains(&("Audio/Música".into(), GroupKind::Music)));
        assert!(k.contains(&("Audio/SFX".into(), GroupKind::Sfx)));
        assert!(k.contains(&("Audio/Locución".into(), GroupKind::Voiceover)));
        assert!(k.contains(&("Renders".into(), GroupKind::Renders)));
        assert!(k.contains(&("Proxies".into(), GroupKind::Proxies)));
        assert!(k.contains(&("Stock".into(), GroupKind::Stock)));
        assert!(k.contains(&("Graficos".into(), GroupKind::Graphics)));
    }

    #[test]
    fn detects_already_organized_projects() {
        let i = inv(&["1. imagen/a.png", "2. video/b.mov", "3. audio/c.wav", "4. editable/p.aep"]);
        let g = detect_groups(&i, "25.11.26 X - Y", &default_render_patterns());
        assert_eq!(kinds(&g), vec![("".into(), GroupKind::OrganizedProject)]);
    }

    #[test]
    fn render_pattern_matching_is_by_whole_word() {
        let p = default_render_patterns();
        assert!(matches_render_pattern("Spot_FINAL_v2.mov", &p));
        assert!(matches_render_pattern("master 4k.mov", &p));
        assert!(!matches_render_pattern("finalcut_notes.mov", &p));
        assert!(!matches_render_pattern("A001_C002.mov", &p));
    }

    #[test]
    fn anchor_is_deepest_enabled_group() {
        let i = inv(&["Audio/Música/Temas/a.wav"]);
        let g = detect_groups(&i, "P", &default_render_patterns());
        let a = find_anchor(&g, "Audio/Música/Temas").unwrap();
        assert_eq!(a.kind, GroupKind::Music);
        assert_eq!(rel_within(&a.rel_dir, "Audio/Música/Temas"), "Temas");
        assert!(find_anchor(&g, "Otra").is_none());
    }
}
