//! Duplicados por contenido y conflictos de nombres, y conversión del plan en
//! operaciones de copia definitivas.
//!
//! Nunca se sobrescribe un archivo existente: un conflicto con contenido
//! distinto exige una decisión (omitir, conservar ambos o renombrar la copia).

use crate::error::{CoreError, CoreResult};
use crate::hashing::{partial_hash, sha256_file};
use crate::paths::{compare_key, join_rel, name_issues, with_suffix};
use crate::plan::{Plan, PlanItem};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DuplicatePolicy {
    /// Copiar todos los archivos aunque tengan contenido idéntico (recomendado para no romper vínculos).
    #[default]
    CopyAll,
    /// Copiar solo la primera aparición de cada contenido idéntico.
    SkipIdentical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DefaultConflictAction {
    #[default]
    Ask,
    Skip,
    KeepBoth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Resolution {
    Skip,
    KeepBoth,
    Rename { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    /// Ya existe en destino un archivo con el mismo nombre y contenido idéntico.
    ExistingIdentical,
    /// Ya existe en destino un archivo con el mismo nombre y contenido diferente.
    ExistingDifferent,
    /// Dos orígenes van al mismo destino con contenido idéntico.
    InternalIdentical,
    /// Dos orígenes van al mismo destino con contenido diferente.
    InternalDifferent,
}

impl ConflictKind {
    pub fn label(self) -> &'static str {
        match self {
            ConflictKind::ExistingIdentical => "Ya existe en el destino (idéntico)",
            ConflictKind::ExistingDifferent => "Ya existe en el destino (contenido diferente)",
            ConflictKind::InternalIdentical => "Mismo nombre y contenido en el origen",
            ConflictKind::InternalDifferent => "Mismo nombre, contenido diferente",
        }
    }
    pub fn identical(self) -> bool {
        matches!(self, ConflictKind::ExistingIdentical | ConflictKind::InternalIdentical)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub path: String,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub id: u32,
    pub kind: ConflictKind,
    pub item_id: u32,
    pub name: String,
    /// Elemento del plan con el que choca (conflictos internos).
    pub other_item_id: Option<u32>,
    pub dest_rel: String,
    pub source: FileStat,
    /// Archivo existente en destino o el otro origen.
    pub other: FileStat,
    pub comparison: String,
    pub needs_decision: bool,
    pub resolution: Option<Resolution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub hash: String,
    pub size: u64,
    pub item_ids: Vec<u32>,
    pub rels: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub duplicates: Vec<DuplicateGroup>,
    pub conflicts: Vec<Conflict>,
    pub hashed_files: usize,
    pub hashed_bytes: u64,
    /// Hash SHA-256 de origen ya calculados (se reutilizan durante la copia).
    #[serde(skip)]
    pub source_hashes: HashMap<u32, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgress {
    pub phase: String,
    pub done: usize,
    pub total: usize,
}

fn stat_of(path: &Path) -> Option<FileStat> {
    let m = std::fs::metadata(crate::paths::long_path(path)).ok()?;
    Some(FileStat {
        path: path.display().to_string(),
        size: m.len(),
        mtime: m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    })
}

struct Hasher<'a> {
    cancel: &'a AtomicBool,
    full: HashMap<PathBuf, String>,
    partial: HashMap<PathBuf, String>,
    files: usize,
    bytes: u64,
}

impl<'a> Hasher<'a> {
    fn full(&mut self, p: &Path) -> CoreResult<String> {
        if let Some(h) = self.full.get(p) {
            return Ok(h.clone());
        }
        let mut b = 0;
        let h = sha256_file(p, self.cancel, |n| b += n).map_err(|e| {
            if e.kind() == std::io::ErrorKind::Interrupted {
                CoreError::Cancelled
            } else {
                CoreError::io(p, e)
            }
        })?;
        self.files += 1;
        self.bytes += b;
        self.full.insert(p.to_path_buf(), h.clone());
        Ok(h)
    }
    fn partial(&mut self, p: &Path) -> CoreResult<String> {
        if let Some(h) = self.partial.get(p) {
            return Ok(h.clone());
        }
        let h = partial_hash(p).map_err(|e| CoreError::io(p, e))?;
        self.partial.insert(p.to_path_buf(), h.clone());
        Ok(h)
    }
    /// Compara contenido: tamaño → hash parcial → SHA-256 completo.
    fn same_content(&mut self, a: &Path, a_size: u64, b: &Path, b_size: u64) -> CoreResult<bool> {
        if a_size != b_size {
            return Ok(false);
        }
        if a_size == 0 {
            return Ok(true);
        }
        if self.partial(a)? != self.partial(b)? {
            return Ok(false);
        }
        Ok(self.full(a)? == self.full(b)?)
    }
}

/// Analiza duplicados por contenido y conflictos de nombres.
pub fn analyze(
    plan: &Plan,
    dest_root: &Path,
    detect_content_duplicates: bool,
    default_action: DefaultConflictAction,
    cancel: &AtomicBool,
    mut progress: impl FnMut(AnalysisProgress),
) -> CoreResult<Analysis> {
    let included: Vec<&PlanItem> = plan.items.iter().filter(|i| i.included()).collect();
    let mut h = Hasher { cancel, full: HashMap::new(), partial: HashMap::new(), files: 0, bytes: 0 };
    let mut analysis = Analysis::default();

    // 1. Duplicados por contenido: tamaño → hash parcial → SHA-256.
    if detect_content_duplicates {
        let mut by_size: BTreeMap<u64, Vec<&PlanItem>> = BTreeMap::new();
        for i in &included {
            if i.size > 0 {
                by_size.entry(i.size).or_default().push(i);
            }
        }
        let candidates: Vec<Vec<&PlanItem>> = by_size.into_values().filter(|v| v.len() > 1).collect();
        let total: usize = candidates.iter().map(|v| v.len()).sum();
        let mut done = 0;
        for group in candidates {
            let mut by_partial: HashMap<String, Vec<&PlanItem>> = HashMap::new();
            for i in &group {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err(CoreError::Cancelled);
                }
                match h.partial(&i.source) {
                    Ok(p) => by_partial.entry(p).or_default().push(i),
                    Err(_) => continue, // Ilegible: lo reportará la verificación previa.
                }
                done += 1;
                progress(AnalysisProgress { phase: "Buscando duplicados".into(), done, total });
            }
            for (_, same) in by_partial.into_iter().filter(|(_, v)| v.len() > 1) {
                let mut by_full: BTreeMap<String, Vec<&PlanItem>> = BTreeMap::new();
                for i in same {
                    if let Ok(full) = h.full(&i.source) {
                        by_full.entry(full).or_default().push(i);
                    }
                }
                for (hash, items) in by_full.into_iter().filter(|(_, v)| v.len() > 1) {
                    let mut items = items;
                    items.sort_by_key(|i| i.id);
                    analysis.duplicates.push(DuplicateGroup {
                        hash,
                        size: items[0].size,
                        item_ids: items.iter().map(|i| i.id).collect(),
                        rels: items.iter().map(|i| i.rel.clone()).collect(),
                    });
                }
            }
        }
    }

    // 2. Conflictos de nombre (claves NFC + minúsculas, como en volúmenes insensibles).
    let mut by_dest: BTreeMap<String, Vec<&PlanItem>> = BTreeMap::new();
    for i in &included {
        by_dest.entry(compare_key(&i.dest_rel)).or_default().push(i);
    }
    let total = by_dest.len();
    let mut next_id = 0u32;
    let suggestion = |kind: ConflictKind| -> (bool, Option<Resolution>) {
        if kind.identical() {
            (false, Some(Resolution::Skip))
        } else {
            match default_action {
                DefaultConflictAction::Ask => (true, None),
                DefaultConflictAction::Skip => (true, Some(Resolution::Skip)),
                DefaultConflictAction::KeepBoth => (true, Some(Resolution::KeepBoth)),
            }
        }
    };
    for (n, (_, mut items)) in by_dest.into_iter().enumerate() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(CoreError::Cancelled);
        }
        if n % 200 == 0 {
            progress(AnalysisProgress { phase: "Comprobando conflictos".into(), done: n, total });
        }
        items.sort_by_key(|i| i.id);
        let first = items[0];
        let dest = join_rel(dest_root, &first.dest_rel);
        let existing = std::fs::symlink_metadata(crate::paths::long_path(&dest)).ok();
        let src_stat = |i: &PlanItem| FileStat { path: i.source.display().to_string(), size: i.size, mtime: i.mtime };

        if let Some(meta) = existing {
            // Todos los orígenes que apuntan aquí chocan con el archivo existente.
            let ex_stat = stat_of(&dest).unwrap_or(FileStat { path: dest.display().to_string(), size: 0, mtime: 0 });
            for i in &items {
                let (kind, comparison) = if meta.is_dir() {
                    (ConflictKind::ExistingDifferent, "En el destino existe una carpeta con ese nombre".to_string())
                } else if h.same_content(&i.source, i.size, &dest, meta.len()).unwrap_or(false) {
                    (ConflictKind::ExistingIdentical, "Contenido idéntico (SHA-256)".to_string())
                } else if i.size != meta.len() {
                    (ConflictKind::ExistingDifferent, "Tamaño diferente".to_string())
                } else {
                    (ConflictKind::ExistingDifferent, "Mismo tamaño, contenido diferente (SHA-256)".to_string())
                };
                let (needs_decision, resolution) = suggestion(kind);
                analysis.conflicts.push(Conflict {
                    id: next_id,
                    kind,
                    item_id: i.id,
                    name: i.name.clone(),
                    other_item_id: None,
                    dest_rel: i.dest_rel.clone(),
                    source: src_stat(i),
                    other: ex_stat.clone(),
                    comparison,
                    needs_decision,
                    resolution,
                });
                next_id += 1;
            }
            continue;
        }
        // Conflictos internos: el primero conserva el nombre.
        for i in items.iter().skip(1) {
            let same = h.same_content(&i.source, i.size, &first.source, first.size).unwrap_or(false);
            let (kind, comparison) = if same {
                (ConflictKind::InternalIdentical, "Contenido idéntico (SHA-256)".to_string())
            } else if i.size != first.size {
                (ConflictKind::InternalDifferent, "Tamaño diferente".to_string())
            } else {
                (ConflictKind::InternalDifferent, "Mismo tamaño, contenido diferente".to_string())
            };
            let (needs_decision, resolution) = suggestion(kind);
            analysis.conflicts.push(Conflict {
                id: next_id,
                kind,
                item_id: i.id,
                name: i.name.clone(),
                other_item_id: Some(first.id),
                dest_rel: i.dest_rel.clone(),
                source: src_stat(i),
                other: src_stat(first),
                comparison,
                needs_decision,
                resolution,
            });
            next_id += 1;
        }
    }
    analysis.hashed_files = h.files;
    analysis.hashed_bytes = h.bytes;
    // Guardar hashes de origen para reutilizarlos.
    let by_path: HashMap<&Path, u32> = included.iter().map(|i| (i.source.as_path(), i.id)).collect();
    for (p, hash) in h.full {
        if let Some(id) = by_path.get(p.as_path()) {
            analysis.source_hashes.insert(*id, hash);
        }
    }
    Ok(analysis)
}

// ---------------------------------------------------------------------------
// Operaciones finales
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OpAction {
    Copy,
    Skip { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub idx: u32,
    pub item_id: u32,
    pub src: PathBuf,
    pub rel_src: String,
    pub dest_rel: String,
    pub size: u64,
    pub mtime: i64,
    pub category: crate::Category,
    pub action: OpAction,
    /// Nombre de destino distinto al original (conservar ambos / renombrar).
    pub renamed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedEntry {
    pub item_id: u32,
    pub rel_src: String,
    pub size: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationSet {
    pub ops: Vec<Operation>,
    pub dirs: Vec<String>,
    pub excluded: Vec<ExcludedEntry>,
}

/// Convierte plan + análisis + decisiones en operaciones. Falla si quedan
/// conflictos de contenido diferente sin resolver.
pub fn build_operations(
    plan: &Plan,
    analysis: &Analysis,
    resolutions: &HashMap<u32, Resolution>,
    policy: DuplicatePolicy,
    dest_root: &Path,
) -> CoreResult<OperationSet> {
    let mut conflict_by_item: HashMap<u32, &Conflict> = HashMap::new();
    for c in &analysis.conflicts {
        conflict_by_item.insert(c.item_id, c);
    }
    let unresolved: Vec<&Conflict> = analysis
        .conflicts
        .iter()
        .filter(|c| c.needs_decision && resolutions.get(&c.id).or(c.resolution.as_ref()).is_none())
        .collect();
    if !unresolved.is_empty() {
        return Err(CoreError::Invalid(format!(
            "Hay {} conflicto(s) sin resolver; elige omitir, conservar ambos o renombrar",
            unresolved.len()
        )));
    }
    let mut skip_dup: HashMap<u32, String> = HashMap::new();
    if policy == DuplicatePolicy::SkipIdentical {
        for g in &analysis.duplicates {
            for (id, _) in g.item_ids.iter().zip(&g.rels).skip(1) {
                skip_dup.insert(*id, format!("Duplicado idéntico de «{}»", g.rels[0]));
            }
        }
    }

    let mut taken: HashSet<String> = plan
        .items
        .iter()
        .filter(|i| i.included())
        .map(|i| compare_key(&i.dest_rel))
        .collect();
    let mut ops = Vec::new();
    let mut excluded = Vec::new();
    for item in &plan.items {
        if let Some(r) = item.excluded {
            excluded.push(ExcludedEntry {
                item_id: item.id,
                rel_src: item.rel.clone(),
                size: item.size,
                reason: r.label().to_string(),
            });
            continue;
        }
        let mut dest_rel = item.dest_rel.clone();
        let mut renamed = false;
        let mut action = OpAction::Copy;
        if let Some(reason) = skip_dup.get(&item.id) {
            action = OpAction::Skip { reason: reason.clone() };
        } else if let Some(c) = conflict_by_item.get(&item.id) {
            let res = resolutions.get(&c.id).cloned().or_else(|| c.resolution.clone()).unwrap_or(Resolution::Skip);
            match res {
                Resolution::Skip => {
                    let reason = match c.kind {
                        ConflictKind::ExistingIdentical => "Ya existe un archivo idéntico en el destino".to_string(),
                        ConflictKind::InternalIdentical => "Duplicado idéntico con el mismo destino".to_string(),
                        _ => "Omitido por conflicto de nombre".to_string(),
                    };
                    action = OpAction::Skip { reason };
                }
                Resolution::KeepBoth => {
                    let parent = crate::plan::dest_parent(&item.dest_rel).to_string();
                    let mut n = 1;
                    loop {
                        let candidate_name = with_suffix(&item.name, &format!("_copia{n}"));
                        let candidate = if parent.is_empty() { candidate_name } else { format!("{parent}/{candidate_name}") };
                        if !taken.contains(&compare_key(&candidate)) && !join_rel(dest_root, &candidate).exists() {
                            dest_rel = candidate;
                            break;
                        }
                        n += 1;
                    }
                    renamed = true;
                    taken.insert(compare_key(&dest_rel));
                }
                Resolution::Rename { name } => {
                    let name = name.trim().to_string();
                    if name.is_empty() || name.contains('/') || name.contains('\\') || !name_issues(&name).is_empty() {
                        return Err(CoreError::Invalid(format!("El nombre «{name}» no es válido")));
                    }
                    let parent = crate::plan::dest_parent(&item.dest_rel).to_string();
                    let candidate = if parent.is_empty() { name.clone() } else { format!("{parent}/{name}") };
                    if taken.contains(&compare_key(&candidate)) || join_rel(dest_root, &candidate).exists() {
                        return Err(CoreError::Invalid(format!("Ya existe un archivo llamado «{name}» en el destino")));
                    }
                    taken.insert(compare_key(&candidate));
                    dest_rel = candidate;
                    renamed = true;
                }
            }
        }
        ops.push(Operation {
            idx: ops.len() as u32,
            item_id: item.id,
            src: item.source.clone(),
            rel_src: item.rel.clone(),
            dest_rel,
            size: item.size,
            mtime: item.mtime,
            category: item.category,
            action,
            renamed,
        });
    }
    Ok(OperationSet { ops, dirs: plan.dirs_to_create.clone(), excluded })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{build_plan, Overrides, PlanOptions};
    use crate::scan::scan;

    fn setup(files: &[(&str, &[u8])]) -> (tempfile::TempDir, Plan) {
        let d = tempfile::tempdir().unwrap();
        for (p, c) in files {
            let path = d.path().join("src").join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, c).unwrap();
        }
        std::fs::create_dir_all(d.path().join("dest")).unwrap();
        let inv = scan(&[d.path().join("src")], &AtomicBool::new(false), |_, _| {}).unwrap();
        let plan = build_plan(&inv, &PlanOptions { preserve_subfolders: false, ..Default::default() }, &Overrides::default());
        (d, plan)
    }

    fn run(d: &tempfile::TempDir, plan: &Plan) -> Analysis {
        analyze(plan, &d.path().join("dest"), true, DefaultConflictAction::Ask, &AtomicBool::new(false), |_| {}).unwrap()
    }

    #[test]
    fn detects_identical_content_with_different_names() {
        let (d, plan) = setup(&[("a.mov", b"same content"), ("b/otro.mov", b"same content"), ("c.mov", b"diff content")]);
        let a = run(&d, &plan);
        assert_eq!(a.duplicates.len(), 1);
        assert_eq!(a.duplicates[0].item_ids.len(), 2);
        assert!(a.conflicts.is_empty());
        let ops = build_operations(&plan, &a, &HashMap::new(), DuplicatePolicy::SkipIdentical, &d.path().join("dest")).unwrap();
        let skipped = ops.ops.iter().filter(|o| matches!(o.action, OpAction::Skip { .. })).count();
        assert_eq!(skipped, 1);
        let ops = build_operations(&plan, &a, &HashMap::new(), DuplicatePolicy::CopyAll, &d.path().join("dest")).unwrap();
        assert!(ops.ops.iter().all(|o| o.action == OpAction::Copy));
    }

    #[test]
    fn same_name_different_content_requires_decision() {
        let (d, plan) = setup(&[("x/clip.mov", b"AAAA"), ("y/clip.mov", b"BBBB"), ("z/CLIP.MOV", b"AAAA")]);
        let a = run(&d, &plan);
        let kinds: Vec<ConflictKind> = a.conflicts.iter().map(|c| c.kind).collect();
        assert!(kinds.contains(&ConflictKind::InternalDifferent));
        assert!(kinds.contains(&ConflictKind::InternalIdentical));
        let dest = d.path().join("dest");
        assert!(build_operations(&plan, &a, &HashMap::new(), DuplicatePolicy::CopyAll, &dest).is_err());
        let diff = a.conflicts.iter().find(|c| c.kind == ConflictKind::InternalDifferent).unwrap();
        let mut res = HashMap::new();
        res.insert(diff.id, Resolution::KeepBoth);
        let ops = build_operations(&plan, &a, &res, DuplicatePolicy::CopyAll, &dest).unwrap();
        let dests: Vec<&str> = ops.ops.iter().filter(|o| o.action == OpAction::Copy).map(|o| o.dest_rel.as_str()).collect();
        assert!(dests.contains(&"2. video/clip.mov"));
        assert!(dests.contains(&"2. video/clip_copia1.mov"));
        assert_eq!(ops.ops.iter().filter(|o| matches!(o.action, OpAction::Skip { .. })).count(), 1);
    }

    #[test]
    fn existing_destination_files_are_never_overwritten() {
        let (d, plan) = setup(&[("a.wav", b"nuevo"), ("b.wav", b"igual")]);
        let dest = d.path().join("dest");
        std::fs::create_dir_all(dest.join("3. audio")).unwrap();
        std::fs::write(dest.join("3. audio/a.wav"), b"viejo").unwrap();
        std::fs::write(dest.join("3. audio/b.wav"), b"igual").unwrap();
        let a = run(&d, &plan);
        let ca = a.conflicts.iter().find(|c| c.name == "a.wav").unwrap();
        let cb = a.conflicts.iter().find(|c| c.name == "b.wav").unwrap();
        assert_eq!(ca.kind, ConflictKind::ExistingDifferent);
        assert!(ca.needs_decision);
        assert_eq!(cb.kind, ConflictKind::ExistingIdentical);
        assert!(!cb.needs_decision);
        let mut res = HashMap::new();
        res.insert(ca.id, Resolution::Rename { name: "a nueva.wav".into() });
        let ops = build_operations(&plan, &a, &res, DuplicatePolicy::CopyAll, &dest).unwrap();
        let oa = ops.ops.iter().find(|o| o.rel_src == "a.wav").unwrap();
        assert_eq!(oa.dest_rel, "3. audio/a nueva.wav");
        let ob = ops.ops.iter().find(|o| o.rel_src == "b.wav").unwrap();
        assert!(matches!(ob.action, OpAction::Skip { .. }));
        // Renombrar a un nombre existente se rechaza.
        res.insert(ca.id, Resolution::Rename { name: "b.wav".into() });
        assert!(build_operations(&plan, &a, &res, DuplicatePolicy::CopyAll, &dest).is_err());
        res.insert(ca.id, Resolution::Rename { name: "mal:nombre.wav".into() });
        assert!(build_operations(&plan, &a, &res, DuplicatePolicy::CopyAll, &dest).is_err());
    }

    #[test]
    fn default_action_prefills_resolution() {
        let (d, plan) = setup(&[("x/clip.mov", b"AAAA"), ("y/clip.mov", b"BBBBB")]);
        let a = analyze(&plan, &d.path().join("dest"), true, DefaultConflictAction::KeepBoth, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(a.conflicts[0].resolution, Some(Resolution::KeepBoth));
        assert!(build_operations(&plan, &a, &HashMap::new(), DuplicatePolicy::CopyAll, &d.path().join("dest")).is_ok());
    }
}
