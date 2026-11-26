use crate::state::{err, AppState, Session};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use wpo_core::duplicates::{self, Conflict, DuplicateGroup, Resolution};
use wpo_core::folders::FolderGroup;
use wpo_core::history::{self, HistoryEntry};
use wpo_core::naming::{self, ParsedProjectName};
use wpo_core::plan::{
    self, build_plan, ItemOverride, ItemPage, ItemQuery, PlanOptions, PlanSequence, PlanSummary, PlanWarning, TreeNode,
};
use wpo_core::preflight::{self, PreflightReport};
use wpo_core::settings::{self, AppSettings};
use wpo_core::structure::{self, StructureCheck, StructureResult};
use wpo_core::Category;

type CmdResult<T> = Result<T, String>;

// ---------------------------------------------------------------------------
// Configuración
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> AppSettings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, settings: AppSettings) -> CmdResult<AppSettings> {
    let mut s = settings;
    s.validate().map_err(err)?;
    settings::save(&state.settings_path(), &s).map_err(err)?;
    *state.settings.lock().unwrap() = s.clone();
    Ok(s)
}

#[tauri::command]
pub fn reset_settings(state: State<AppState>) -> CmdResult<AppSettings> {
    let current = state.settings.lock().unwrap().clone();
    let s = AppSettings {
        recent_destinations: current.recent_destinations,
        frequent_paths: current.frequent_paths,
        ..Default::default()
    };
    settings::save(&state.settings_path(), &s).map_err(err)?;
    *state.settings.lock().unwrap() = s.clone();
    Ok(s)
}

fn remember_destination(state: &AppState, path: &Path) {
    let mut s = state.settings.lock().unwrap();
    s.remember_destination(&path.display().to_string());
    let _ = settings::save(&state.settings_path(), &s);
}

// ---------------------------------------------------------------------------
// Crear estructura
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamePreview {
    pub name: String,
    pub problems: Vec<String>,
}

#[tauri::command]
pub fn preview_project_name(state: State<AppState>, date: String, client: String, project: String) -> CmdResult<NamePreview> {
    let d = naming::parse_iso_date(&date).ok_or("Fecha no válida")?;
    let s = state.settings.lock().unwrap().naming.clone();
    let name = naming::format_project_name(&s, d, &client, &project);
    let mut problems = Vec::new();
    if client.trim().is_empty() {
        problems.push("Falta el nombre del cliente".to_string());
    }
    if project.trim().is_empty() {
        problems.push("Falta el nombre del proyecto".to_string());
    }
    if name.len() > 120 {
        problems.push("El nombre es muy largo; puede causar rutas largas en Windows".to_string());
    }
    Ok(NamePreview { name, problems })
}

#[tauri::command]
pub fn parse_project_name(name: String) -> Option<ParsedProjectName> {
    naming::parse_project_name(&name)
}

#[tauri::command]
pub fn check_structure(parent: String, name: String) -> StructureCheck {
    structure::check_structure(Path::new(&parent), &name)
}

#[tauri::command]
pub fn create_structure(
    state: State<AppState>,
    parent: String,
    name: String,
    client: String,
    project: String,
    complete_existing: bool,
) -> CmdResult<StructureResult> {
    let parent = PathBuf::from(parent);
    let r = structure::create_structure(&parent, &name, complete_existing).map_err(|e| match e {
        wpo_core::CoreError::AlreadyExists(p) => format!("Ya existe una carpeta llamada «{}» en este destino", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
        wpo_core::CoreError::NotFound(p) => format!("La ubicación «{}» no existe o no está conectada", p.display()),
        other => other.to_string(),
    })?;
    remember_destination(&state, &parent);
    let _ = history::upsert(
        &state.history_path(),
        HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            date: wpo_core::copy::now_iso(),
            client,
            project,
            kind: "structure".into(),
            destination: r.root.display().to_string(),
            files: 0,
            size: 0,
            status: "created".into(),
            job_id: None,
        },
    );
    Ok(r)
}

// ---------------------------------------------------------------------------
// Sistema de archivos
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> CmdResult<()> {
    if !Path::new(&path).exists() {
        return Err(format!("La ruta «{path}» no existe o la unidad no está conectada"));
    }
    app.opener().open_path(path, None::<&str>).map_err(err)
}

#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> CmdResult<()> {
    if !Path::new(&path).exists() {
        return Err(format!("La ruta «{path}» no existe o la unidad no está conectada"));
    }
    app.opener().reveal_item_in_dir(path).map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInfo {
    pub exists: bool,
    pub is_dir: bool,
    pub free_bytes: Option<u64>,
    pub filesystem: Option<String>,
}

#[tauri::command]
pub fn path_info(path: String) -> PathInfo {
    let p = Path::new(&path);
    PathInfo {
        exists: p.exists(),
        is_dir: p.is_dir(),
        free_bytes: preflight::available_space(p),
        filesystem: preflight::filesystem_type(p),
    }
}

#[tauri::command]
pub fn list_locations(app: AppHandle, state: State<AppState>) -> Vec<crate::locations::Location> {
    let s = state.settings.lock().unwrap().clone();
    let frequent: Vec<(String, String)> = s.frequent_paths.iter().map(|f| (f.label.clone(), f.path.clone())).collect();
    crate::locations::list(&app, &s.recent_destinations, &frequent)
}

// ---------------------------------------------------------------------------
// Organizar: escaneo y plan
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanResponse {
    pub session_id: String,
    pub root_name: String,
    pub single_root: bool,
    pub sources: Vec<String>,
    pub options: PlanOptions,
    pub summary: PlanSummary,
    pub warnings: Vec<PlanWarning>,
    pub sequences: Vec<PlanSequence>,
    pub groups: Vec<FolderGroup>,
    pub dirs_to_create: Vec<String>,
    pub suggested_name: Option<ParsedProjectName>,
}

fn plan_response(s: &Session) -> PlanResponse {
    let root_name = if s.inventory.single_root {
        s.inventory.sources[0].file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    } else {
        String::new()
    };
    let sequences = s
        .plan
        .sequences
        .iter()
        .map(|q| {
            let mut q = q.clone();
            q.info.file_ids.clear();
            q
        })
        .collect();
    PlanResponse {
        session_id: s.id.clone(),
        suggested_name: naming::parse_project_name(&root_name),
        root_name,
        single_root: s.inventory.single_root,
        sources: s.inventory.sources.iter().map(|p| p.display().to_string()).collect(),
        options: s.options.clone(),
        summary: s.plan.summary.clone(),
        warnings: s.plan.warnings.clone(),
        sequences,
        groups: s.plan.groups.clone(),
        dirs_to_create: s.plan.dirs_to_create.clone(),
    }
}

fn rebuild(s: &mut Session) {
    s.plan = build_plan(&s.inventory, &s.options, &s.overrides);
    s.analysis = None;
    s.preflight = None;
    s.resolutions.clear();
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScanProgress {
    files: usize,
    bytes: u64,
}

#[tauri::command]
pub async fn scan_sources(app: AppHandle, state: State<'_, AppState>, paths: Vec<String>, options: PlanOptions) -> CmdResult<PlanResponse> {
    let cancel = state.scan_cancel.clone();
    cancel.store(false, Ordering::SeqCst);
    let sources: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let app2 = app.clone();
    let used_options = options.clone();
    let (inventory, plan) = tauri::async_runtime::spawn_blocking(move || {
        let mut last = std::time::Instant::now();
        let inv = wpo_core::scan::scan(&sources, &cancel, |files, bytes| {
            if last.elapsed().as_millis() > 150 {
                last = std::time::Instant::now();
                let _ = app2.emit("scan-progress", ScanProgress { files, bytes });
            }
        })?;
        let plan = build_plan(&inv, &options, &Default::default());
        Ok::<_, wpo_core::CoreError>((inv, plan))
    })
    .await
    .map_err(err)?
    .map_err(|e| match e {
        wpo_core::CoreError::Cancelled => "Análisis cancelado".to_string(),
        other => other.to_string(),
    })?;
    let options = used_options;
    let session = Session {
        id: uuid::Uuid::new_v4().to_string(),
        inventory,
        options,
        overrides: Default::default(),
        plan,
        analysis: None,
        preflight: None,
        dest_root: None,
        resolutions: HashMap::new(),
    };
    let resp = plan_response(&session);
    *state.session.lock().unwrap() = Some(session);
    Ok(resp)
}

#[tauri::command]
pub fn cancel_scan(state: State<AppState>) {
    state.scan_cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn close_session(state: State<AppState>) {
    *state.session.lock().unwrap() = None;
}

fn with_session<T>(state: &AppState, f: impl FnOnce(&mut Session) -> CmdResult<T>) -> CmdResult<T> {
    let mut guard = state.session.lock().unwrap();
    let s = guard.as_mut().ok_or("No hay un análisis activo. Vuelve a seleccionar los archivos.")?;
    f(s)
}

#[tauri::command]
pub async fn update_plan(state: State<'_, AppState>, options: PlanOptions) -> CmdResult<PlanResponse> {
    with_session(&state, |s| {
        s.options = options;
        rebuild(s);
        Ok(plan_response(s))
    })
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct OverridePatch {
    pub category: Option<Category>,
    pub render: Option<bool>,
    pub include: Option<bool>,
    pub subfolder: Option<String>,
    /// Elimina todos los cambios manuales de estos elementos.
    pub reset: bool,
}

#[tauri::command]
pub async fn set_item_overrides(state: State<'_, AppState>, ids: Vec<u32>, patch: OverridePatch) -> CmdResult<PlanResponse> {
    with_session(&state, |s| {
        for id in ids {
            if patch.reset {
                s.overrides.items.remove(&id);
                continue;
            }
            let o: &mut ItemOverride = s.overrides.items.entry(id).or_default();
            if patch.category.is_some() {
                o.category = patch.category;
                o.subfolder = None;
            }
            if patch.render.is_some() {
                o.render = patch.render;
            }
            if patch.include.is_some() {
                o.include = patch.include;
            }
            if let Some(sub) = &patch.subfolder {
                let clean: Vec<String> = sub
                    .split(['/', '\\'])
                    .map(|c| c.trim())
                    .filter(|c| !c.is_empty() && *c != "." && *c != "..")
                    .map(wpo_core::paths::sanitize_component)
                    .collect();
                o.subfolder = if clean.is_empty() { None } else { Some(clean.join("/")) };
            }
        }
        rebuild(s);
        Ok(plan_response(s))
    })
}

#[tauri::command]
pub async fn set_sequence_decision(state: State<'_, AppState>, keys: Vec<String>, accept: Option<bool>) -> CmdResult<PlanResponse> {
    with_session(&state, |s| {
        for k in keys {
            match accept {
                Some(v) => {
                    s.overrides.sequence_decisions.insert(k, v);
                }
                None => {
                    s.overrides.sequence_decisions.remove(&k);
                }
            }
        }
        rebuild(s);
        Ok(plan_response(s))
    })
}

#[tauri::command]
pub async fn set_group_enabled(state: State<'_, AppState>, rel_dir: String, enabled: bool) -> CmdResult<PlanResponse> {
    with_session(&state, |s| {
        if enabled {
            s.overrides.disabled_groups.remove(&rel_dir);
        } else {
            s.overrides.disabled_groups.insert(rel_dir);
        }
        rebuild(s);
        Ok(plan_response(s))
    })
}

#[tauri::command]
pub async fn query_items(state: State<'_, AppState>, query: ItemQuery) -> CmdResult<ItemPage> {
    with_session(&state, |s| Ok(plan::query_items(&s.plan, &query)))
}

#[tauri::command]
pub async fn tree_children(state: State<'_, AppState>, dir: String) -> CmdResult<Vec<TreeNode>> {
    with_session(&state, |s| Ok(plan::tree_children(&s.plan, &dir)))
}

#[tauri::command]
pub async fn get_plan_details(state: State<'_, AppState>) -> CmdResult<PlanResponse> {
    with_session(&state, |s| Ok(plan_response(s)))
}

// ---------------------------------------------------------------------------
// Verificación: duplicados, conflictos y comprobaciones previas
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResponse {
    pub dest_root: String,
    pub duplicates: Vec<DuplicateGroup>,
    pub conflicts: Vec<Conflict>,
    pub unresolved: usize,
    pub preflight: PreflightReport,
    pub hashed_files: usize,
    pub hashed_bytes: u64,
}

fn effective_conflicts(conflicts: &[Conflict], res: &HashMap<u32, Resolution>) -> Vec<Conflict> {
    conflicts
        .iter()
        .map(|c| {
            let mut c = c.clone();
            if let Some(r) = res.get(&c.id) {
                c.resolution = Some(r.clone());
            }
            c
        })
        .collect()
}

fn unresolved_count(conflicts: &[Conflict]) -> usize {
    conflicts.iter().filter(|c| c.needs_decision && c.resolution.is_none()).count()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AnalysisProgressEvent {
    phase: String,
    done: usize,
    total: usize,
}

#[tauri::command]
pub async fn run_analysis(app: AppHandle, state: State<'_, AppState>, dest_root: String) -> CmdResult<AnalysisResponse> {
    let dest = PathBuf::from(dest_root.trim());
    if dest.as_os_str().is_empty() {
        return Err("Selecciona una ubicación de destino".into());
    }
    let (session_id, plan_clone, sources) = with_session(&state, |s| {
        Ok((s.id.clone(), s.plan.clone(), s.inventory.sources.clone()))
    })?;
    let settings = state.settings.lock().unwrap().clone();
    let cancel = state.analysis_cancel.clone();
    cancel.store(false, Ordering::SeqCst);
    let dest2 = dest.clone();
    let app2 = app.clone();
    let (analysis, preflight) = tauri::async_runtime::spawn_blocking(move || {
        let mut last = std::time::Instant::now();
        let analysis = duplicates::analyze(
            &plan_clone,
            &dest2,
            settings.duplicates.detect_content,
            settings.duplicates.default_conflict_action,
            &cancel,
            |p| {
                if last.elapsed().as_millis() > 150 {
                    last = std::time::Instant::now();
                    let _ = app2.emit("analysis-progress", AnalysisProgressEvent { phase: p.phase, done: p.done, total: p.total });
                }
            },
        )?;
        // Para la verificación previa, los conflictos pendientes se cuentan como omitidos.
        let provisional: HashMap<u32, Resolution> = analysis
            .conflicts
            .iter()
            .filter(|c| c.resolution.is_none())
            .map(|c| (c.id, Resolution::Skip))
            .collect();
        let ops = duplicates::build_operations(&plan_clone, &analysis, &provisional, settings.duplicates.policy, &dest2)?;
        let mut last = std::time::Instant::now();
        let pf = preflight::run_preflight(&ops, &sources, &dest2, &cancel, |done, total| {
            if last.elapsed().as_millis() > 150 {
                last = std::time::Instant::now();
                let _ = app2.emit("analysis-progress", AnalysisProgressEvent { phase: "Verificando permisos y rutas".into(), done, total });
            }
        });
        if cancel.load(Ordering::SeqCst) {
            return Err(wpo_core::CoreError::Cancelled);
        }
        Ok::<_, wpo_core::CoreError>((analysis, pf))
    })
    .await
    .map_err(err)?
    .map_err(|e| match e {
        wpo_core::CoreError::Cancelled => "Verificación cancelada".to_string(),
        other => other.to_string(),
    })?;

    with_session(&state, |s| {
        if s.id != session_id {
            return Err("El análisis cambió mientras se verificaba; vuelve a intentarlo".into());
        }
        let conflicts = effective_conflicts(&analysis.conflicts, &HashMap::new());
        let resp = AnalysisResponse {
            dest_root: dest.display().to_string(),
            duplicates: analysis.duplicates.clone(),
            unresolved: unresolved_count(&conflicts),
            conflicts,
            preflight: preflight.clone(),
            hashed_files: analysis.hashed_files,
            hashed_bytes: analysis.hashed_bytes,
        };
        s.analysis = Some(analysis);
        s.preflight = Some(preflight);
        s.dest_root = Some(dest.clone());
        s.resolutions.clear();
        Ok(resp)
    })
}

#[tauri::command]
pub fn cancel_analysis(state: State<AppState>) {
    state.analysis_cancel.store(true, Ordering::SeqCst);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionInput {
    pub id: u32,
    pub resolution: Option<Resolution>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictsResponse {
    pub conflicts: Vec<Conflict>,
    pub unresolved: usize,
}

#[tauri::command]
pub async fn resolve_conflicts(state: State<'_, AppState>, items: Vec<ResolutionInput>, apply_to_equivalent: bool) -> CmdResult<ConflictsResponse> {
    with_session(&state, |s| {
        let analysis = s.analysis.as_ref().ok_or("Primero ejecuta la verificación")?;
        for it in items {
            let Some(c) = analysis.conflicts.iter().find(|c| c.id == it.id) else { continue };
            let targets: Vec<u32> = if apply_to_equivalent {
                // Equivalentes: mismo tipo de conflicto (no se aplica "renombrar" en bloque).
                analysis
                    .conflicts
                    .iter()
                    .filter(|o| o.kind == c.kind && !matches!(it.resolution, Some(Resolution::Rename { .. })) || o.id == c.id)
                    .map(|o| o.id)
                    .collect()
            } else {
                vec![c.id]
            };
            for t in targets {
                match &it.resolution {
                    Some(r) => {
                        s.resolutions.insert(t, r.clone());
                    }
                    None => {
                        s.resolutions.remove(&t);
                    }
                }
            }
        }
        let conflicts = effective_conflicts(&analysis.conflicts, &s.resolutions);
        Ok(ConflictsResponse { unresolved: unresolved_count(&conflicts), conflicts })
    })
}

// ---------------------------------------------------------------------------
// Historial
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn history_list(state: State<AppState>) -> Vec<HistoryEntry> {
    history::load(&state.history_path()).entries
}

/// Elimina registros del historial (y opcionalmente sus reportes guardados en
/// los datos de la aplicación). Nunca toca los archivos de los proyectos.
#[tauri::command]
pub fn history_delete(state: State<AppState>, ids: Vec<String>, delete_reports: bool) -> CmdResult<usize> {
    let h = history::load(&state.history_path());
    let running: Vec<String> = state.jobs.lock().unwrap().keys().cloned().collect();
    let ids: Vec<String> = ids
        .into_iter()
        .filter(|id| {
            h.entries
                .iter()
                .find(|e| &e.id == id)
                .and_then(|e| e.job_id.as_ref())
                .map(|j| !running.contains(j))
                .unwrap_or(true)
        })
        .collect();
    if delete_reports {
        for e in h.entries.iter().filter(|e| ids.contains(&e.id)) {
            if let Some(j) = &e.job_id {
                let dir = state.job_dir(j);
                // Solo se borra la carpeta interna del trabajo dentro de los datos de la app.
                if dir.starts_with(state.jobs_dir()) && dir.is_dir() {
                    let _ = std::fs::remove_dir_all(dir);
                }
            }
        }
    }
    history::delete(&state.history_path(), &ids).map_err(err)
}
