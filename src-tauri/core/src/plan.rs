//! Plan de organización: dónde terminará cada archivo en la copia.
//!
//! `build_plan(inventario, opciones, overrides)` es una función pura: la
//! interfaz puede cambiar opciones o decisiones y recalcular sin volver a
//! escanear el disco.

use crate::classify::{extension_of, is_adobe_project_ext, Classifier, CustomRule};
use crate::exclusions::{self, ExclusionReason, ExclusionSettings};
use crate::folders::{self, FolderGroup, GroupKind};
use crate::paths::compare_key;
use crate::scan::Inventory;
use crate::sequence::{self, SequenceConfidence, SequenceInfo, SequenceSettings};
use crate::Category;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const RENDERS_FOLDER: &str = "Renders finales";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum OrganizeMode {
    /// Clasificar en la estructura de cinco carpetas.
    #[default]
    Organize,
    /// Copia de seguridad conservando la jerarquía original.
    PreserveHierarchy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PlanOptions {
    pub mode: OrganizeMode,
    /// Conservar las subcarpetas originales dentro de cada categoría.
    pub preserve_subfolders: bool,
    pub exclusions: ExclusionSettings,
    pub sequences: SequenceSettings,
    pub render_patterns: Vec<String>,
    /// Crear `2. video/Renders finales` aunque no se detecten renders.
    pub always_create_render_folder: bool,
    pub custom_rules: Vec<CustomRule>,
}

impl Default for PlanOptions {
    fn default() -> Self {
        PlanOptions {
            mode: OrganizeMode::Organize,
            preserve_subfolders: true,
            exclusions: ExclusionSettings::default(),
            sequences: SequenceSettings::default(),
            render_patterns: folders::default_render_patterns(),
            always_create_render_folder: false,
            custom_rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ItemOverride {
    pub category: Option<Category>,
    pub render: Option<bool>,
    pub include: Option<bool>,
    /// Subcarpeta dentro de la categoría (con `/`), p. ej. `Material/Día 1`.
    pub subfolder: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Overrides {
    pub items: HashMap<u32, ItemOverride>,
    /// Decisión del usuario por clave de secuencia: `true` = es secuencia.
    pub sequence_decisions: HashMap<String, bool>,
    /// Carpetas funcionales que el usuario prefiere tratar como normales.
    pub disabled_groups: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub id: u32,
    pub source: PathBuf,
    pub rel: String,
    pub name: String,
    pub ext: String,
    pub size: u64,
    pub mtime: i64,
    pub category: Category,
    /// Ruta relativa a la raíz del proyecto de destino, con `/`.
    pub dest_rel: String,
    pub reason: String,
    pub sequence_key: Option<String>,
    pub group_dir: Option<String>,
    pub render: bool,
    pub render_suggested: bool,
    pub excluded: Option<ExclusionReason>,
    pub unknown_ext: bool,
    pub overridden: bool,
}

impl PlanItem {
    pub fn included(&self) -> bool {
        self.excluded.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSequence {
    #[serde(flatten)]
    pub info: SequenceInfo,
    /// `Some(true)` aceptada, `Some(false)` rechazada, `None` pendiente (solo ambiguas).
    pub decision: Option<bool>,
    pub accepted: bool,
    pub dest_dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WarningLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanWarning {
    pub code: String,
    pub level: WarningLevel,
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryStat {
    pub count: usize,
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExclusionStat {
    pub reason: Option<ExclusionReason>,
    pub label: String,
    pub count: usize,
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSummary {
    pub mode: OrganizeMode,
    pub total_files: usize,
    pub total_size: u64,
    pub included_files: usize,
    pub included_size: u64,
    pub by_category: BTreeMap<String, CategoryStat>,
    pub excluded: Vec<ExclusionStat>,
    pub excluded_files: usize,
    pub excluded_size: u64,
    pub sequences: usize,
    pub sequences_pending: usize,
    pub renders: usize,
    pub unknown_ext: usize,
    pub adobe_projects: usize,
    pub groups: usize,
    pub already_organized: bool,
    pub scan_errors: usize,
    pub symlinks: usize,
    pub empty_dirs: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub items: Vec<PlanItem>,
    pub sequences: Vec<PlanSequence>,
    pub groups: Vec<FolderGroup>,
    /// Carpetas (relativas al destino) que se crearán aunque estén vacías.
    pub dirs_to_create: Vec<String>,
    pub warnings: Vec<PlanWarning>,
    pub summary: PlanSummary,
}

/// Componentes de carpeta que repiten el nombre de una categoría y se omiten
/// al conservar subcarpetas (`Video/clip.mov` → `2. video/clip.mov`).
fn is_category_synonym(name: &str) -> bool {
    if Category::from_folder_name(name).is_some() {
        return true;
    }
    matches!(
        compare_key(name).as_str(),
        "imagen" | "imagenes" | "imágenes" | "images" | "image" | "img" | "video" | "videos" | "vídeo"
            | "vídeos" | "audio" | "audios" | "editable" | "editables" | "archivos" | "files"
    )
}

fn clean_subpath(rel_dir: &str) -> String {
    let parts: Vec<&str> = rel_dir.split('/').filter(|s| !s.is_empty()).collect();
    let skip = parts.iter().take_while(|p| is_category_synonym(p)).count();
    parts[skip..].join("/")
}

fn join(parts: &[&str]) -> String {
    parts
        .iter()
        .flat_map(|p| p.split('/'))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn parent_of(rel: &str) -> &str {
    match rel.rfind('/') {
        Some(i) => &rel[..i],
        None => "",
    }
}

/// Fuerza la exclusividad de la carpeta editable.
fn enforce_editable(cat: Category, ext: &str) -> Category {
    let adobe = is_adobe_project_ext(ext);
    if adobe {
        Category::Editable
    } else if cat == Category::Editable {
        Category::Archivos
    } else {
        cat
    }
}

pub fn build_plan(inv: &Inventory, opts: &PlanOptions, ov: &Overrides) -> Plan {
    let classifier = Classifier::new(&opts.custom_rules);
    let root_name = if inv.single_root {
        inv.sources
            .first()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        String::new()
    };

    // 1. Exclusiones.
    let excluded: Vec<Option<ExclusionReason>> = inv
        .files
        .iter()
        .map(|f| {
            let ext = extension_of(Path::new(f.name()));
            let auto = exclusions::evaluate(&opts.exclusions, &f.rel, &ext, f.hidden);
            match ov.items.get(&f.id).and_then(|o| o.include) {
                Some(true) => None,
                Some(false) => Some(auto.unwrap_or(ExclusionReason::Manual)),
                None => auto,
            }
        })
        .collect();

    // 2. Grupos funcionales de carpetas.
    let mut groups = folders::detect_groups(inv, &root_name, &opts.render_patterns);
    for g in groups.iter_mut() {
        g.enabled = !ov.disabled_groups.contains(&g.rel_dir);
    }

    // 3. Secuencias (solo imágenes no excluidas).
    let mut dir_counts: HashMap<String, usize> = HashMap::new();
    for (f, ex) in inv.files.iter().zip(&excluded) {
        if ex.is_none() {
            *dir_counts.entry(f.rel_dir().to_string()).or_default() += 1;
        }
    }
    let candidates: Vec<sequence::Candidate> = inv
        .files
        .iter()
        .zip(&excluded)
        .filter(|(f, ex)| ex.is_none() && classifier.classify_path(Path::new(f.name())) == Category::Imagen)
        .map(|(f, _)| sequence::Candidate { id: f.id, rel_dir: f.rel_dir(), name: f.name(), size: f.size })
        .collect();
    let detected = sequence::detect_sequences(&candidates, &dir_counts, &opts.sequences);
    let mut seq_by_file: HashMap<u32, usize> = HashMap::new();
    let mut used_seq_dirs: HashSet<String> = HashSet::new();
    let mut sequences: Vec<PlanSequence> = Vec::new();
    for info in detected {
        let decision = ov.sequence_decisions.get(&info.key).copied();
        let accepted = match info.confidence {
            SequenceConfidence::High => decision.unwrap_or(true),
            SequenceConfidence::Ambiguous => decision.unwrap_or(false),
        };
        // Nombre de carpeta único dentro de 1. imagen.
        let mut name = info.folder_name.clone();
        let mut n = 2;
        while used_seq_dirs.contains(&compare_key(&name)) {
            name = format!("{} ({n})", info.folder_name);
            n += 1;
        }
        if accepted {
            used_seq_dirs.insert(compare_key(&name));
            for id in &info.file_ids {
                seq_by_file.insert(*id, sequences.len());
            }
        }
        sequences.push(PlanSequence { decision, accepted, dest_dir: name, info });
    }

    // 4. Destino de cada archivo.
    let mut items = Vec::with_capacity(inv.files.len());
    for (f, ex) in inv.files.iter().zip(excluded) {
        let name = f.name().to_string();
        let ext = extension_of(Path::new(&name));
        let o = ov.items.get(&f.id);
        let base_cat = classifier.classify_ext(&ext);
        let mut category = base_cat;
        let mut overridden = false;
        if let Some(c) = o.and_then(|o| o.category) {
            if classifier.category_allowed(&ext, c) {
                category = c;
                overridden = c != base_cat;
            }
        }
        let rel_dir = f.rel_dir();
        let anchor = folders::find_anchor(&groups, rel_dir);
        let render_by_name = folders::matches_render_pattern(&name, &opts.render_patterns);
        let render_by_dir = anchor.map(|a| a.kind == GroupKind::Renders).unwrap_or(false);
        let render_suggested = category == Category::Video && (render_by_name || render_by_dir);
        let render = category == Category::Video && o.and_then(|o| o.render).unwrap_or(render_suggested);
        if o.and_then(|o| o.render).is_some() {
            overridden = true;
        }

        let (dest_rel, reason, group_dir, seq_key) = if opts.mode == OrganizeMode::PreserveHierarchy {
            (f.rel.clone(), "Jerarquía original conservada".to_string(), None, None)
        } else if let Some(sub) = o.and_then(|o| o.subfolder.clone()) {
            overridden = true;
            (
                join(&[category.folder_name(), &sub, &name]),
                "Destino elegido manualmente".to_string(),
                None,
                None,
            )
        } else {
            place(f, &name, &ext, &mut category, render, anchor, &seq_by_file, &sequences, opts, overridden)
        };

        items.push(PlanItem {
            id: f.id,
            source: f.abs.clone(),
            rel: f.rel.clone(),
            unknown_ext: classifier.is_unknown(&ext),
            name,
            ext,
            size: f.size,
            mtime: f.mtime,
            category,
            dest_rel,
            reason,
            sequence_key: seq_key,
            group_dir,
            render,
            render_suggested,
            excluded: ex,
            overridden,
        });
    }

    // 5. Carpetas a crear.
    let mut dirs: Vec<String> = Vec::new();
    if opts.mode == OrganizeMode::Organize {
        for c in Category::ALL {
            dirs.push(c.folder_name().to_string());
        }
        let has_render = items.iter().any(|i| i.included() && i.render);
        if opts.always_create_render_folder || has_render {
            dirs.push(join(&[Category::Video.folder_name(), RENDERS_FOLDER]));
        }
    }
    if !opts.exclusions.empty_folders {
        for d in inv.dirs.iter().filter(|d| d.is_empty) {
            if exclusions::evaluate(&opts.exclusions, &format!("{}/x", d.rel), "", false).is_some() {
                continue;
            }
            match opts.mode {
                OrganizeMode::PreserveHierarchy => dirs.push(d.rel.clone()),
                OrganizeMode::Organize => {
                    // Solo dentro de estructuras que se conservan completas.
                    if let Some(a) = folders::find_anchor(&groups, &d.rel) {
                        if a.kind == GroupKind::CameraCard {
                            let cat = a.category.unwrap_or(Category::Video);
                            dirs.push(join(&[cat.folder_name(), &a.name, &folders::rel_within(&a.rel_dir, &d.rel)]));
                        } else if a.kind == GroupKind::OrganizedProject {
                            let within = folders::rel_within(&a.rel_dir, &d.rel);
                            if let Some((first, rest)) = within.split_once('/') {
                                if let Some(c) = Category::from_folder_name(first) {
                                    dirs.push(join(&[c.folder_name(), rest]));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    dirs.sort();
    dirs.dedup();

    let mut plan = Plan { items, sequences, groups, dirs_to_create: dirs, warnings: Vec::new(), summary: Default::default() };
    plan.summary = summarize(&plan, inv, opts);
    plan.warnings = warnings_for(&plan, inv, opts);
    plan
}

#[allow(clippy::too_many_arguments)]
fn place(
    f: &crate::scan::FileEntry,
    name: &str,
    ext: &str,
    category: &mut Category,
    render: bool,
    anchor: Option<&FolderGroup>,
    seq_by_file: &HashMap<u32, usize>,
    sequences: &[PlanSequence],
    opts: &PlanOptions,
    user_category: bool,
) -> (String, String, Option<String>, Option<String>) {
    let rel_dir = f.rel_dir();
    let sub = |s: &str| if opts.preserve_subfolders { clean_subpath(s) } else { String::new() };

    // Estructuras que se conservan completas.
    if let Some(a) = anchor {
        let within = folders::rel_within(&a.rel_dir, rel_dir);
        match a.kind {
            GroupKind::OrganizedProject => {
                let (first, rest) = within.split_once('/').unwrap_or((within.as_str(), ""));
                if let Some(c) = Category::from_folder_name(first) {
                    if !user_category {
                        *category = enforce_editable(c, ext);
                    }
                    let reason = if *category == c {
                        "Proyecto ya organizado: se conserva su ubicación".to_string()
                    } else {
                        "Proyecto ya organizado: reubicado por la regla de editables".to_string()
                    };
                    return (join(&[category.folder_name(), rest, name]), reason, Some(a.rel_dir.clone()), None);
                }
                // Archivos fuera de las carpetas estándar: tratamiento normal relativo a la raíz.
                return (
                    join(&[category.folder_name(), &sub(&within), name]),
                    format!("Extensión .{ext}"),
                    Some(a.rel_dir.clone()),
                    None,
                );
            }
            GroupKind::CameraCard => {
                let card_cat = a.category.unwrap_or(Category::Video);
                if !user_category {
                    *category = enforce_editable(card_cat, ext);
                }
                return (
                    join(&[category.folder_name(), &a.name, &within, name]),
                    format!("{}: estructura conservada", a.detail),
                    Some(a.rel_dir.clone()),
                    None,
                );
            }
            _ => {}
        }
    }

    // Secuencias de imágenes.
    if let Some(si) = seq_by_file.get(&f.id) {
        let s = &sequences[*si];
        return (
            join(&[category.folder_name(), &s.dest_dir, name]),
            format!("Secuencia de imágenes ({} fotogramas)", s.info.count),
            anchor.map(|a| a.rel_dir.clone()),
            Some(s.info.key.clone()),
        );
    }

    // Renders finales.
    if render && *category == Category::Video {
        let within = anchor
            .filter(|a| a.kind == GroupKind::Renders)
            .map(|a| folders::rel_within(&a.rel_dir, rel_dir))
            .unwrap_or_default();
        return (
            join(&[Category::Video.folder_name(), RENDERS_FOLDER, &within, name]),
            "Render final".to_string(),
            anchor.map(|a| a.rel_dir.clone()),
            None,
        );
    }

    // Carpetas funcionales.
    if let Some(a) = anchor {
        let within = folders::rel_within(&a.rel_dir, rel_dir);
        let dest = match (a.kind, *category) {
            (GroupKind::Proxies, Category::Video) => Some(join(&[Category::Video.folder_name(), "Proxies", &within, name])),
            (GroupKind::Music, Category::Audio) => Some(join(&[Category::Audio.folder_name(), "Música", &within, name])),
            (GroupKind::Sfx, Category::Audio) => Some(join(&[Category::Audio.folder_name(), "SFX", &within, name])),
            (GroupKind::Voiceover, Category::Audio) => {
                Some(join(&[Category::Audio.folder_name(), "Locuciones", &within, name]))
            }
            (GroupKind::Stock, c) => Some(join(&[c.folder_name(), "Stock", &within, name])),
            (GroupKind::Graphics, c) => Some(join(&[c.folder_name(), &a.name, &within, name])),
            (GroupKind::Autosave, c) => Some(join(&[c.folder_name(), "Auto-Save", &within, name])),
            _ => None,
        };
        if let Some(d) = dest {
            return (d, format!("{} ({})", a.kind.label(), a.name), Some(a.rel_dir.clone()), None);
        }
    }

    let reason = if user_category {
        "Categoría elegida manualmente".to_string()
    } else {
        format!("Extensión .{}", if ext.is_empty() { "(sin extensión)" } else { ext })
    };
    (join(&[category.folder_name(), &sub(rel_dir), name]), reason, None, None)
}

fn summarize(plan: &Plan, inv: &Inventory, opts: &PlanOptions) -> PlanSummary {
    let mut s = PlanSummary { mode: opts.mode, ..Default::default() };
    for c in Category::ALL {
        s.by_category.insert(c.key().to_string(), CategoryStat::default());
    }
    let mut ex: BTreeMap<String, ExclusionStat> = BTreeMap::new();
    for i in &plan.items {
        s.total_files += 1;
        s.total_size += i.size;
        match i.excluded {
            Some(r) => {
                s.excluded_files += 1;
                s.excluded_size += i.size;
                let e = ex.entry(r.label().to_string()).or_insert_with(|| ExclusionStat {
                    reason: Some(r),
                    label: r.label().to_string(),
                    ..Default::default()
                });
                e.count += 1;
                e.size += i.size;
            }
            None => {
                s.included_files += 1;
                s.included_size += i.size;
                let c = s.by_category.get_mut(i.category.key()).unwrap();
                c.count += 1;
                c.size += i.size;
                if i.render {
                    s.renders += 1;
                }
                if i.unknown_ext {
                    s.unknown_ext += 1;
                }
                if i.category == Category::Editable {
                    s.adobe_projects += 1;
                }
            }
        }
    }
    s.excluded = ex.into_values().collect();
    s.sequences = plan.sequences.iter().filter(|q| q.accepted).count();
    s.sequences_pending = plan
        .sequences
        .iter()
        .filter(|q| q.info.confidence == SequenceConfidence::Ambiguous && q.decision.is_none())
        .count();
    s.groups = plan.groups.len();
    s.already_organized = plan.groups.iter().any(|g| g.kind == GroupKind::OrganizedProject && g.rel_dir.is_empty());
    s.scan_errors = inv.errors.len();
    s.symlinks = inv.symlinks.len();
    s.empty_dirs = inv.dirs.iter().filter(|d| d.is_empty).count();
    s
}

fn warnings_for(plan: &Plan, inv: &Inventory, opts: &PlanOptions) -> Vec<PlanWarning> {
    let mut w = Vec::new();
    let s = &plan.summary;
    if s.adobe_projects > 0 {
        if opts.mode == OrganizeMode::Organize {
            w.push(PlanWarning {
                code: "adobe-relink".into(),
                level: WarningLevel::Critical,
                title: format!("{} proyecto(s) de After Effects / Premiere Pro", s.adobe_projects),
                message: "Los archivos .aep, .aepx y .prproj guardan rutas a sus recursos. Al reorganizar la copia, \
                          los vínculos pueden romperse y será necesario revincular manualmente. Este proceso crea un \
                          backup organizado de archivos, no un proyecto recopilado listo para abrir. Si necesitas \
                          abrir el proyecto sin revincular, usa «Backup conservando jerarquía» o recopila antes con \
                          Collect Files (After Effects) o Project Manager (Premiere Pro). Se generará un mapa de rutas \
                          originales y nuevas para facilitar la revinculación."
                    .into(),
            });
        } else {
            w.push(PlanWarning {
                code: "adobe-hierarchy".into(),
                level: WarningLevel::Info,
                title: "Jerarquía conservada para proyectos de Adobe".into(),
                message: "Se mantiene la estructura original. Los vínculos relativos deberían conservarse; los vínculos \
                          absolutos seguirán apuntando a la ubicación original. Para un proyecto portable usa Collect \
                          Files o Project Manager."
                    .into(),
            });
        }
    }
    if s.already_organized && opts.mode == OrganizeMode::Organize {
        w.push(PlanWarning {
            code: "already-organized".into(),
            level: WarningLevel::Info,
            title: "El proyecto ya sigue la estructura estandarizada".into(),
            message: "Los archivos que ya están en sus carpetas estándar conservan su ubicación; no se reorganizan."
                .into(),
        });
    }
    if s.sequences_pending > 0 {
        w.push(PlanWarning {
            code: "sequences-pending".into(),
            level: WarningLevel::Warning,
            title: format!("{} posible(s) secuencia(s) requieren confirmación", s.sequences_pending),
            message: "Revisa la pestaña Secuencias e indica si cada grupo es una secuencia o son imágenes independientes."
                .into(),
        });
    }
    let incomplete = plan.sequences.iter().filter(|q| q.accepted && q.info.missing_count > 0).count();
    if incomplete > 0 {
        w.push(PlanWarning {
            code: "sequences-incomplete".into(),
            level: WarningLevel::Warning,
            title: format!("{incomplete} secuencia(s) con fotogramas faltantes"),
            message: "Las secuencias se copiarán tal como están en el origen; los fotogramas faltantes se listan en la \
                      pestaña Secuencias."
                .into(),
        });
    }
    if !inv.errors.is_empty() {
        w.push(PlanWarning {
            code: "scan-errors".into(),
            level: WarningLevel::Critical,
            title: format!("{} elemento(s) no se pudieron leer", inv.errors.len()),
            message: inv
                .errors
                .iter()
                .take(5)
                .map(|e| format!("{}: {}", e.path.display(), e.message))
                .collect::<Vec<_>>()
                .join("\n"),
        });
    }
    if !inv.symlinks.is_empty() {
        w.push(PlanWarning {
            code: "symlinks".into(),
            level: WarningLevel::Info,
            title: format!("{} enlace(s) simbólico(s) omitidos", inv.symlinks.len()),
            message: "Los enlaces simbólicos no se siguen ni se copian para evitar duplicar o salir del proyecto.".into(),
        });
    }
    if s.unknown_ext > 0 {
        w.push(PlanWarning {
            code: "unknown-ext".into(),
            level: WarningLevel::Info,
            title: format!("{} archivo(s) sin clasificación específica", s.unknown_ext),
            message: "Las extensiones desconocidas se copian en «5. archivos». Puedes añadir reglas en Configuración.".into(),
        });
    }
    w
}

// ---------------------------------------------------------------------------
// Consultas para la interfaz
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ItemFilter {
    #[default]
    All,
    Included,
    Excluded,
    Render,
    Sequence,
    Unknown,
    Adobe,
    Overridden,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ItemQuery {
    pub category: Option<Category>,
    pub filter: ItemFilter,
    pub search: String,
    /// Prefijo de carpeta de destino (para el árbol).
    pub dest_dir: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemPage {
    pub total: usize,
    pub items: Vec<PlanItem>,
}

pub fn query_items(plan: &Plan, q: &ItemQuery) -> ItemPage {
    let search = compare_key(q.search.trim());
    let limit = if q.limit == 0 { 200 } else { q.limit.min(5000) };
    let prefix = q.dest_dir.as_ref().map(|d| format!("{}/", d.trim_end_matches('/')));
    let mut total = 0;
    let mut out = Vec::new();
    for i in &plan.items {
        if let Some(c) = q.category {
            if i.category != c {
                continue;
            }
        }
        let ok = match q.filter {
            ItemFilter::All => true,
            ItemFilter::Included => i.included(),
            ItemFilter::Excluded => !i.included(),
            ItemFilter::Render => i.render && i.included(),
            ItemFilter::Sequence => i.sequence_key.is_some(),
            ItemFilter::Unknown => i.unknown_ext,
            ItemFilter::Adobe => i.category == Category::Editable,
            ItemFilter::Overridden => i.overridden,
        };
        if !ok {
            continue;
        }
        if let Some(p) = &prefix {
            if !i.dest_rel.starts_with(p.as_str()) {
                continue;
            }
        }
        if !search.is_empty() && !compare_key(&i.rel).contains(&search) && !compare_key(&i.dest_rel).contains(&search) {
            continue;
        }
        if total >= q.offset && out.len() < limit {
            out.push(i.clone());
        }
        total += 1;
    }
    ItemPage { total, items: out }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub file_count: usize,
    pub size: u64,
    pub item_id: Option<u32>,
    pub category: Option<Category>,
    pub render: bool,
}

/// Hijos directos de una carpeta del árbol de destino (`""` = raíz).
pub fn tree_children(plan: &Plan, dir: &str) -> Vec<TreeNode> {
    let prefix = if dir.is_empty() { String::new() } else { format!("{}/", dir.trim_end_matches('/')) };
    let mut dirs: BTreeMap<String, TreeNode> = BTreeMap::new();
    let mut files: Vec<TreeNode> = Vec::new();
    for d in &plan.dirs_to_create {
        if let Some(rest) = d.strip_prefix(prefix.as_str()) {
            if rest.is_empty() {
                continue;
            }
            let first = rest.split('/').next().unwrap();
            dirs.entry(first.to_string()).or_insert_with(|| TreeNode {
                name: first.to_string(),
                path: format!("{prefix}{first}"),
                is_dir: true,
                file_count: 0,
                size: 0,
                item_id: None,
                category: None,
                render: false,
            });
        }
    }
    for i in plan.items.iter().filter(|i| i.included()) {
        let Some(rest) = i.dest_rel.strip_prefix(prefix.as_str()) else { continue };
        match rest.split_once('/') {
            Some((first, _)) => {
                let n = dirs.entry(first.to_string()).or_insert_with(|| TreeNode {
                    name: first.to_string(),
                    path: format!("{prefix}{first}"),
                    is_dir: true,
                    file_count: 0,
                    size: 0,
                    item_id: None,
                    category: None,
                    render: false,
                });
                n.file_count += 1;
                n.size += i.size;
            }
            None => files.push(TreeNode {
                name: rest.to_string(),
                path: i.dest_rel.clone(),
                is_dir: false,
                file_count: 1,
                size: i.size,
                item_id: Some(i.id),
                category: Some(i.category),
                render: i.render,
            }),
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out: Vec<TreeNode> = dirs.into_values().collect();
    out.extend(files);
    out
}

/// Ruta absoluta de destino de un elemento.
pub fn dest_abs(dest_root: &Path, dest_rel: &str) -> PathBuf {
    crate::paths::join_rel(dest_root, dest_rel)
}

/// Carpeta padre relativa de un destino.
pub fn dest_parent(dest_rel: &str) -> &str {
    parent_of(dest_rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::FileEntry;

    fn inv(paths: &[&str]) -> Inventory {
        let files = paths
            .iter()
            .enumerate()
            .map(|(i, p)| FileEntry {
                id: i as u32,
                abs: format!("/src/P/{p}").into(),
                rel: p.to_string(),
                size: 100,
                mtime: 0,
                hidden: false,
            })
            .collect();
        Inventory { sources: vec!["/src/P".into()], files, single_root: true, ..Default::default() }
    }

    fn dest(plan: &Plan, rel: &str) -> String {
        plan.items.iter().find(|i| i.rel == rel).unwrap().dest_rel.clone()
    }

    #[test]
    fn classifies_into_five_folders_preserving_meaningful_subfolders() {
        let i = inv(&["Video/Dia 1/A001.MOV", "foto.jpg", "Proyecto.aep", "doc.pdf", "raro.xyz", "pista.wav"]);
        let p = build_plan(&i, &PlanOptions::default(), &Overrides::default());
        assert_eq!(dest(&p, "Video/Dia 1/A001.MOV"), "2. video/Dia 1/A001.MOV");
        assert_eq!(dest(&p, "foto.jpg"), "1. imagen/foto.jpg");
        assert_eq!(dest(&p, "Proyecto.aep"), "4. editable/Proyecto.aep");
        assert_eq!(dest(&p, "doc.pdf"), "5. archivos/doc.pdf");
        assert_eq!(dest(&p, "raro.xyz"), "5. archivos/raro.xyz");
        assert_eq!(dest(&p, "pista.wav"), "3. audio/pista.wav");
        assert_eq!(p.summary.unknown_ext, 1);
        assert_eq!(p.dirs_to_create.len(), 5);
        assert!(p.warnings.iter().any(|w| w.code == "adobe-relink"));
    }

    #[test]
    fn flatten_option() {
        let i = inv(&["Material/Dia 1/A001.mov"]);
        let o = PlanOptions { preserve_subfolders: false, ..Default::default() };
        let p = build_plan(&i, &o, &Overrides::default());
        assert_eq!(dest(&p, "Material/Dia 1/A001.mov"), "2. video/A001.mov");
    }

    #[test]
    fn sequences_get_their_own_folder_and_keep_names() {
        let names: Vec<String> = (1..=30).map(|n| format!("Comp/Explosion_{n:04}.png")).collect();
        let mut paths: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        paths.push("Comp/notas.txt");
        let p = build_plan(&inv(&paths), &PlanOptions::default(), &Overrides::default());
        assert_eq!(dest(&p, "Comp/Explosion_0001.png"), "1. imagen/Explosion/Explosion_0001.png");
        assert_eq!(dest(&p, "Comp/Explosion_0030.png"), "1. imagen/Explosion/Explosion_0030.png");
        assert_eq!(p.summary.sequences, 1);
    }

    #[test]
    fn sequence_in_its_own_folder_keeps_folder_name() {
        let names: Vec<String> = (1..=10).map(|n| format!("renders/Humo_v2/humo.{n:04}.exr")).collect();
        let paths: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let p = build_plan(&inv(&paths), &PlanOptions::default(), &Overrides::default());
        assert_eq!(dest(&p, "renders/Humo_v2/humo.0001.exr"), "1. imagen/Humo_v2/humo.0001.exr");
    }

    #[test]
    fn ambiguous_sequences_need_decision() {
        let names: Vec<String> = (1..=3).map(|n| format!("logo_{n}.png")).collect();
        let paths: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let i = inv(&paths);
        let p = build_plan(&i, &PlanOptions::default(), &Overrides::default());
        assert_eq!(p.summary.sequences_pending, 1);
        assert_eq!(dest(&p, "logo_1.png"), "1. imagen/logo_1.png");
        let mut ov = Overrides::default();
        ov.sequence_decisions.insert(p.sequences[0].info.key.clone(), true);
        let p = build_plan(&i, &PlanOptions::default(), &ov);
        assert_eq!(p.summary.sequences_pending, 0);
        assert_eq!(dest(&p, "logo_1.png"), "1. imagen/logo/logo_1.png");
    }

    #[test]
    fn renders_go_to_renders_folder_and_can_be_toggled() {
        let i = inv(&["Export/Spot 30s.mov", "Spot_FINAL.mp4", "A001.mov"]);
        let p = build_plan(&i, &PlanOptions::default(), &Overrides::default());
        assert_eq!(dest(&p, "Export/Spot 30s.mov"), "2. video/Renders finales/Spot 30s.mov");
        assert_eq!(dest(&p, "Spot_FINAL.mp4"), "2. video/Renders finales/Spot_FINAL.mp4");
        assert_eq!(dest(&p, "A001.mov"), "2. video/A001.mov");
        assert!(p.dirs_to_create.contains(&"2. video/Renders finales".to_string()));

        let mut ov = Overrides::default();
        ov.items.insert(1, ItemOverride { render: Some(false), ..Default::default() });
        ov.items.insert(2, ItemOverride { render: Some(true), ..Default::default() });
        let p = build_plan(&i, &PlanOptions::default(), &ov);
        assert_eq!(dest(&p, "Spot_FINAL.mp4"), "2. video/Spot_FINAL.mp4");
        assert_eq!(dest(&p, "A001.mov"), "2. video/Renders finales/A001.mov");
    }

    #[test]
    fn no_render_folder_without_renders_unless_configured() {
        let i = inv(&["A001.mov"]);
        let p = build_plan(&i, &PlanOptions::default(), &Overrides::default());
        assert!(!p.dirs_to_create.iter().any(|d| d.contains(RENDERS_FOLDER)));
        let o = PlanOptions { always_create_render_folder: true, ..Default::default() };
        let p = build_plan(&i, &o, &Overrides::default());
        assert!(p.dirs_to_create.iter().any(|d| d.contains(RENDERS_FOLDER)));
    }

    #[test]
    fn manual_category_cannot_violate_editable_exclusivity() {
        let i = inv(&["a.psd", "b.aep"]);
        let mut ov = Overrides::default();
        ov.items.insert(0, ItemOverride { category: Some(Category::Editable), ..Default::default() });
        ov.items.insert(1, ItemOverride { category: Some(Category::Archivos), ..Default::default() });
        let p = build_plan(&i, &PlanOptions::default(), &ov);
        assert_eq!(dest(&p, "a.psd"), "5. archivos/a.psd");
        assert_eq!(dest(&p, "b.aep"), "4. editable/b.aep");
        ov.items.insert(0, ItemOverride { category: Some(Category::Imagen), ..Default::default() });
        let p = build_plan(&i, &PlanOptions::default(), &ov);
        assert_eq!(dest(&p, "a.psd"), "1. imagen/a.psd");
    }

    #[test]
    fn camera_cards_are_preserved_whole() {
        let i = inv(&[
            "CardA/PRIVATE/M4ROOT/CLIP/C0001.MP4",
            "CardA/PRIVATE/M4ROOT/CLIP/C0001M01.XML",
            "CardA/DCIM/a.jpg",
        ]);
        let p = build_plan(&i, &PlanOptions::default(), &Overrides::default());
        assert_eq!(dest(&p, "CardA/PRIVATE/M4ROOT/CLIP/C0001.MP4"), "2. video/CardA/PRIVATE/M4ROOT/CLIP/C0001.MP4");
        assert_eq!(dest(&p, "CardA/PRIVATE/M4ROOT/CLIP/C0001M01.XML"), "2. video/CardA/PRIVATE/M4ROOT/CLIP/C0001M01.XML");
        assert_eq!(dest(&p, "CardA/DCIM/a.jpg"), "2. video/CardA/DCIM/a.jpg");
        // El usuario puede desactivar el grupo.
        let mut ov = Overrides::default();
        ov.disabled_groups.insert("CardA".into());
        let p = build_plan(&i, &PlanOptions::default(), &ov);
        assert_eq!(dest(&p, "CardA/PRIVATE/M4ROOT/CLIP/C0001M01.XML"), "5. archivos/CardA/PRIVATE/M4ROOT/CLIP/C0001M01.XML");
    }

    #[test]
    fn already_organized_projects_are_not_reorganized() {
        let i = inv(&["1. imagen/x/a.png", "2. video/Renders finales/b.mov", "3. audio/c.wav", "4. editable/p.aep", "4. editable/ref.psd"]);
        let p = build_plan(&i, &PlanOptions::default(), &Overrides::default());
        assert!(p.summary.already_organized);
        assert_eq!(dest(&p, "1. imagen/x/a.png"), "1. imagen/x/a.png");
        assert_eq!(dest(&p, "2. video/Renders finales/b.mov"), "2. video/Renders finales/b.mov");
        assert_eq!(dest(&p, "4. editable/p.aep"), "4. editable/p.aep");
        assert_eq!(dest(&p, "4. editable/ref.psd"), "5. archivos/ref.psd");
    }

    #[test]
    fn functional_audio_folders() {
        let i = inv(&["Audio/Musica/tema.wav", "Audio/SFX/whoosh.wav", "VO/toma1.wav", "Stock/clip.mp4"]);
        let p = build_plan(&i, &PlanOptions::default(), &Overrides::default());
        assert_eq!(dest(&p, "Audio/Musica/tema.wav"), "3. audio/Música/tema.wav");
        assert_eq!(dest(&p, "Audio/SFX/whoosh.wav"), "3. audio/SFX/whoosh.wav");
        assert_eq!(dest(&p, "VO/toma1.wav"), "3. audio/Locuciones/toma1.wav");
        assert_eq!(dest(&p, "Stock/clip.mp4"), "2. video/Stock/clip.mp4");
    }

    #[test]
    fn exclusions_and_manual_toggles() {
        let i = inv(&["Adobe After Effects Auto-Save/p auto-save 1.aep", ".DS_Store", "a.mov"]);
        let p = build_plan(&i, &PlanOptions::default(), &Overrides::default());
        assert_eq!(p.summary.excluded_files, 2);
        assert_eq!(p.summary.included_files, 1);
        let mut ov = Overrides::default();
        ov.items.insert(0, ItemOverride { include: Some(true), ..Default::default() });
        ov.items.insert(2, ItemOverride { include: Some(false), ..Default::default() });
        let p = build_plan(&i, &PlanOptions::default(), &ov);
        assert!(p.items[0].included());
        assert_eq!(p.items[0].dest_rel, "4. editable/Auto-Save/p auto-save 1.aep");
        assert_eq!(p.items[2].excluded, Some(ExclusionReason::Manual));
    }

    #[test]
    fn preserve_hierarchy_mode() {
        let i = inv(&["Footage/A001.mov", "Proyecto.aep"]);
        let o = PlanOptions { mode: OrganizeMode::PreserveHierarchy, ..Default::default() };
        let p = build_plan(&i, &o, &Overrides::default());
        assert_eq!(dest(&p, "Footage/A001.mov"), "Footage/A001.mov");
        assert!(p.dirs_to_create.is_empty());
        assert!(p.warnings.iter().any(|w| w.code == "adobe-hierarchy"));
    }

    #[test]
    fn manual_subfolder() {
        let i = inv(&["x.mov"]);
        let mut ov = Overrides::default();
        ov.items.insert(0, ItemOverride { subfolder: Some("Material/Día 1".into()), ..Default::default() });
        let p = build_plan(&i, &PlanOptions::default(), &ov);
        assert_eq!(dest(&p, "x.mov"), "2. video/Material/Día 1/x.mov");
    }

    #[test]
    fn query_and_tree() {
        let i = inv(&["a.mov", "b.mov", "c.wav", ".DS_Store"]);
        let p = build_plan(&i, &PlanOptions::default(), &Overrides::default());
        let page = query_items(&p, &ItemQuery { category: Some(Category::Video), ..Default::default() });
        assert_eq!(page.total, 2);
        let page = query_items(&p, &ItemQuery { search: "C.W".into(), ..Default::default() });
        assert_eq!(page.total, 1);
        let page = query_items(&p, &ItemQuery { filter: ItemFilter::Excluded, ..Default::default() });
        assert_eq!(page.total, 1);
        let root = tree_children(&p, "");
        assert_eq!(root.len(), 5);
        let video = root.iter().find(|n| n.name == "2. video").unwrap();
        assert_eq!(video.file_count, 2);
        let kids = tree_children(&p, "2. video");
        assert_eq!(kids.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), vec!["a.mov", "b.mov"]);
    }
}
