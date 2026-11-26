//! Trabajos de copia en segundo plano: inicio, pausa, cancelación,
//! reanudación tras cierre, reintento de fallidos y reportes.

use crate::state::{err, AppState, RunningJob};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use wpo_core::copy::{self, JobControl, JobManifest, Journal, OpResult, Progress, TransferMode, TransferOptions};
use wpo_core::duplicates::{self, OpAction};
use wpo_core::history::{self, HistoryEntry};
use wpo_core::plan::{OrganizeMode, WarningLevel};
use wpo_core::preflight::{self, IssueLevel};
use wpo_core::report::{self, JobReport, ReportEntry, ReportStatus};
use wpo_core::settings::save_json;
use wpo_core::Category;

type CmdResult<T> = Result<T, String>;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JobProgressEvent {
    job_id: String,
    progress: Progress,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JobFinishedEvent {
    job_id: String,
    status: ReportStatus,
    success: bool,
}

fn kind_label(kind: &str, transfer: TransferMode) -> &'static str {
    match (kind, transfer) {
        ("backup", TransferMode::Copy) => "Backup conservando jerarquía",
        ("backup", TransferMode::Move) => "Mover conservando jerarquía",
        (_, TransferMode::Move) => "Mover y organizar proyecto",
        _ => "Organizar proyecto",
    }
}

/// Tipo de operación para el historial.
fn history_kind(m: &JobManifest) -> String {
    if m.transfer.mode == TransferMode::Move {
        "move".into()
    } else {
        m.kind.clone()
    }
}

fn status_key(s: ReportStatus) -> &'static str {
    match s {
        ReportStatus::Completed => "completed",
        ReportStatus::CompletedWithErrors => "completedWithErrors",
        ReportStatus::Cancelled => "cancelled",
        ReportStatus::Aborted => "aborted",
    }
}

fn manifest_path(state: &AppState, id: &str) -> PathBuf {
    state.job_dir(id).join("manifest.json")
}
fn journal_path(state: &AppState, id: &str) -> PathBuf {
    state.job_dir(id).join("journal.jsonl")
}
fn report_path(state: &AppState, id: &str) -> PathBuf {
    state.job_dir(id).join("report.json")
}

fn load_manifest(state: &AppState, id: &str) -> CmdResult<JobManifest> {
    let p = manifest_path(state, id);
    let s = std::fs::read_to_string(&p).map_err(|_| "No se encontró la información del trabajo".to_string())?;
    serde_json::from_str(&s).map_err(err)
}

fn load_report(state: &AppState, id: &str) -> CmdResult<JobReport> {
    let p = report_path(state, id);
    let s = std::fs::read_to_string(&p).map_err(|_| "Este trabajo todavía no tiene reporte".to_string())?;
    serde_json::from_str(&s).map_err(err)
}

/// Escribe un archivo nuevo sin sobrescribir (añade sufijo si existe).
fn write_new(dir: &Path, name: &str, content: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut candidate = dir.join(name);
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(wpo_core::paths::with_suffix(name, &format!("_{n}")));
        n += 1;
    }
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&candidate)?;
    f.write_all(content.as_bytes())?;
    Ok(candidate)
}

fn write_report_to_destination(manifest: &JobManifest, rep: &JobReport) {
    let dir = if manifest.kind == "backup" {
        manifest.dest_root.join("_Reporte Wonderlust")
    } else {
        manifest.dest_root.join(Category::Archivos.folder_name()).join("_Reporte Wonderlust")
    };
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string();
    let _ = write_new(&dir, &format!("reporte_{stamp}.txt"), &report::to_txt(rep));
    let _ = write_new(&dir, &format!("mapa_de_rutas_{stamp}.csv"), &report::to_path_map(rep));
}

/// Lanza (o relanza) un trabajo en un hilo propio.
fn spawn_job(app: AppHandle, manifest: JobManifest, only: Option<HashSet<u32>>) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let id = manifest.job_id.clone();
    {
        let mut jobs = state.jobs.lock().unwrap();
        if jobs.contains_key(&id) {
            return Err("Este trabajo ya está en ejecución".into());
        }
        jobs.insert(
            id.clone(),
            RunningJob { control: Arc::new(JobControl::default()), last_progress: Arc::new(Mutex::new(None)) },
        );
    }
    let (control, last) = {
        let jobs = state.jobs.lock().unwrap();
        let j = jobs.get(&id).unwrap();
        (j.control.clone(), j.last_progress.clone())
    };
    let journal = journal_path(&state, &id);
    let history_path = state.history_path();
    let report_file = report_path(&state, &id);
    let write_to_dest = state.settings.lock().unwrap().write_report_to_destination;
    let _ = history::upsert(
        &history_path,
        HistoryEntry {
            id: id.clone(),
            date: manifest.created_at.clone(),
            client: manifest.client.clone(),
            project: manifest.project.clone(),
            kind: history_kind(&manifest),
            destination: manifest.dest_root.display().to_string(),
            files: manifest.ops.iter().filter(|o| o.action == OpAction::Copy).count(),
            size: manifest.ops.iter().filter(|o| o.action == OpAction::Copy).map(|o| o.size).sum(),
            status: "running".into(),
            job_id: Some(id.clone()),
        },
    );

    std::thread::Builder::new()
        .name(format!("copia-{id}"))
        .spawn(move || {
            let app2 = app.clone();
            let id2 = id.clone();
            // run_job ya limita los avisos de progreso a ~5 por segundo.
            let outcome = copy::run_job(&manifest, &journal, &control, only.as_ref(), |p| {
                *last.lock().unwrap() = Some(p.clone());
                let _ = app2.emit("job-progress", JobProgressEvent { job_id: id2.clone(), progress: p.clone() });
            });
            let state = app.state::<AppState>();
            let (status, success) = match outcome {
                Ok(out) => {
                    let rep = report::build_report(&manifest, &out, copy::now_iso());
                    let _ = save_json(&report_file, &rep);
                    if write_to_dest && manifest.dest_root.is_dir() {
                        write_report_to_destination(&manifest, &rep);
                    }
                    let _ = history::upsert(
                        &state.history_path(),
                        HistoryEntry {
                            id: id.clone(),
                            date: manifest.created_at.clone(),
                            client: manifest.client.clone(),
                            project: manifest.project.clone(),
                            kind: history_kind(&manifest),
                            destination: manifest.dest_root.display().to_string(),
                            files: rep.totals.copied,
                            size: rep.totals.bytes_copied,
                            status: status_key(rep.status).into(),
                            job_id: Some(id.clone()),
                        },
                    );
                    (rep.status, rep.success)
                }
                Err(e) => {
                    // Error antes de empezar (p. ej. no se pudo crear el destino).
                    let rep = report::build_report(
                        &manifest,
                        &copy::JobOutcome {
                            results: manifest.ops.iter().map(|_| OpResult::Pending).collect(),
                            cancelled: false,
                            aborted_reason: Some(e.to_string()),
                            duration_ms: 0,
                            bytes_copied: 0,
                            pruned_dirs: 0,
                        },
                        copy::now_iso(),
                    );
                    let _ = save_json(&report_file, &rep);
                    let _ = history::upsert(
                        &state.history_path(),
                        HistoryEntry {
                            id: id.clone(),
                            date: manifest.created_at.clone(),
                            client: manifest.client.clone(),
                            project: manifest.project.clone(),
                            kind: history_kind(&manifest),
                            destination: manifest.dest_root.display().to_string(),
                            files: 0,
                            size: 0,
                            status: "aborted".into(),
                            job_id: Some(id.clone()),
                        },
                    );
                    (ReportStatus::Aborted, false)
                }
            };
            state.jobs.lock().unwrap().remove(&id);
            let _ = app.emit("job-finished", JobFinishedEvent { job_id: id.clone(), status, success });
        })
        .map_err(err)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartJobParams {
    pub client: String,
    pub project: String,
    #[serde(default)]
    pub transfer: TransferOptions,
}

#[tauri::command]
pub async fn start_job(app: AppHandle, state: State<'_, AppState>, params: StartJobParams) -> CmdResult<String> {
    let settings = state.settings.lock().unwrap().clone();
    let (plan, analysis, resolutions, dest, sources, mode) = {
        let guard = state.session.lock().unwrap();
        let s = guard.as_ref().ok_or("No hay un análisis activo")?;
        let a = s.analysis.clone().ok_or("Primero ejecuta la verificación")?;
        let d = s.dest_root.clone().ok_or("Selecciona un destino")?;
        (s.plan.clone(), a, s.resolutions.clone(), d, s.inventory.sources.clone(), s.options.mode)
    };
    let ops = duplicates::build_operations(&plan, &analysis, &resolutions, settings.duplicates.policy, &dest).map_err(err)?;
    let params_mode = params.transfer.mode;
    // Repetir la verificación previa justo antes de copiar (la unidad pudo desconectarse).
    let (ops, pf) = tauri::async_runtime::spawn_blocking({
        let sources = sources.clone();
        let dest = dest.clone();
        move || {
            let pf = preflight::run_preflight_for(
                &ops,
                &sources,
                &dest,
                Some(params_mode),
                &std::sync::atomic::AtomicBool::new(false),
                |_, _| {},
            );
            (ops, pf)
        }
    })
    .await
    .map_err(err)?;
    if let Some(b) = pf.issues.iter().find(|i| i.level == IssueLevel::Blocking) {
        return Err(format!("{}: {}", b.title, b.detail));
    }
    let job_id = format!("{}-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"), &uuid::Uuid::new_v4().simple().to_string()[..6]);
    let mut warnings: Vec<String> = plan
        .warnings
        .iter()
        .filter(|w| w.level != WarningLevel::Info)
        .map(|w| format!("{}: {}", w.title, w.message))
        .collect();
    warnings.extend(pf.issues.iter().filter(|i| i.level == IssueLevel::Warning).map(|i| format!("{}: {}", i.title, i.detail)));
    let manifest = JobManifest {
        job_id: job_id.clone(),
        created_at: copy::now_iso(),
        kind: if mode == OrganizeMode::PreserveHierarchy { "backup".into() } else { "organize".into() },
        client: params.client,
        project: params.project,
        dest_root: dest.clone(),
        sources,
        ops: ops.ops,
        dirs: ops.dirs,
        excluded: ops.excluded,
        settings: settings.copy.clone(),
        warnings,
        transfer: params.transfer,
    };
    save_json(&manifest_path(&state, &job_id), &manifest).map_err(err)?;
    if let Some(parent) = dest.parent() {
        let mut s = state.settings.lock().unwrap();
        s.remember_destination(&parent.display().to_string());
        let _ = wpo_core::settings::save(&state.settings_path(), &s);
    }
    spawn_job(app, manifest, None)?;
    Ok(job_id)
}

fn with_job<T>(state: &AppState, id: &str, f: impl FnOnce(&RunningJob) -> T) -> CmdResult<T> {
    let jobs = state.jobs.lock().unwrap();
    jobs.get(id).map(f).ok_or_else(|| "El trabajo no está en ejecución".to_string())
}

#[tauri::command]
pub fn pause_job(state: State<AppState>, job_id: String) -> CmdResult<()> {
    with_job(&state, &job_id, |j| j.control.pause.store(true, Ordering::SeqCst))
}

#[tauri::command]
pub fn resume_job(state: State<AppState>, job_id: String) -> CmdResult<()> {
    with_job(&state, &job_id, |j| j.control.pause.store(false, Ordering::SeqCst))
}

#[tauri::command]
pub fn cancel_job(state: State<AppState>, job_id: String) -> CmdResult<()> {
    with_job(&state, &job_id, |j| {
        j.control.cancel.store(true, Ordering::SeqCst);
        j.control.pause.store(false, Ordering::SeqCst);
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub running: bool,
    pub progress: Option<Progress>,
    pub kind: String,
    pub client: String,
    pub project: String,
    pub dest_root: String,
    pub transfer: TransferOptions,
}

#[tauri::command]
pub fn job_status(state: State<AppState>, job_id: String) -> CmdResult<JobStatus> {
    let m = load_manifest(&state, &job_id)?;
    let jobs = state.jobs.lock().unwrap();
    let running = jobs.get(&job_id);
    Ok(JobStatus {
        running: running.is_some(),
        progress: running.and_then(|j| j.last_progress.lock().unwrap().clone()),
        kind: kind_label(&m.kind, m.transfer.mode).into(),
        client: m.client,
        project: m.project,
        dest_root: m.dest_root.display().to_string(),
        transfer: m.transfer,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterruptedJob {
    pub job_id: String,
    pub created_at: String,
    pub kind: String,
    pub client: String,
    pub project: String,
    pub dest_root: String,
    pub files_done: usize,
    pub files_total: usize,
    pub bytes_total: u64,
    pub reason: String,
}

#[tauri::command]
pub async fn list_interrupted_jobs(state: State<'_, AppState>) -> CmdResult<Vec<InterruptedJob>> {
    let mut out = Vec::new();
    let running: HashSet<String> = state.jobs.lock().unwrap().keys().cloned().collect();
    let Ok(rd) = std::fs::read_dir(state.jobs_dir()) else { return Ok(out) };
    for e in rd.flatten() {
        let id = e.file_name().to_string_lossy().into_owned();
        if running.contains(&id) || e.path().join("discarded").exists() {
            continue;
        }
        let Ok(m) = load_manifest(&state, &id) else { continue };
        let reason = match load_report(&state, &id) {
            Err(_) => "La aplicación se cerró durante la copia".to_string(),
            Ok(r) if matches!(r.status, ReportStatus::Cancelled | ReportStatus::Aborted) => {
                r.aborted_reason.unwrap_or_else(|| "Cancelado por el usuario".into())
            }
            Ok(_) => continue,
        };
        let journal = Journal::load(&journal_path(&state, &id));
        let copies: Vec<_> = m.ops.iter().filter(|o| o.action == OpAction::Copy).collect();
        out.push(InterruptedJob {
            files_done: copies.iter().filter(|o| matches!(journal.get(&o.idx), Some(OpResult::Done { .. }))).count(),
            files_total: copies.len(),
            bytes_total: copies.iter().map(|o| o.size).sum(),
            job_id: id,
            created_at: m.created_at,
            kind: kind_label(&m.kind, m.transfer.mode).into(),
            client: m.client,
            project: m.project,
            dest_root: m.dest_root.display().to_string(),
            reason,
        });
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

#[tauri::command]
pub async fn resume_interrupted_job(app: AppHandle, state: State<'_, AppState>, job_id: String) -> CmdResult<()> {
    let m = load_manifest(&state, &job_id)?;
    if !m.dest_root.parent().map(|p| p.is_dir()).unwrap_or(false) {
        return Err(format!("El destino «{}» no está disponible. Conecta la unidad e inténtalo de nuevo.", m.dest_root.display()));
    }
    // Solo se eliminan temporales propios (.wlpart) del destino.
    copy::cleanup_temp_files(&m.dest_root);
    let _ = std::fs::remove_file(report_path(&state, &job_id));
    spawn_job(app, m, None)
}

#[tauri::command]
pub fn discard_interrupted_job(state: State<AppState>, job_id: String) -> CmdResult<()> {
    let dir = state.job_dir(&job_id);
    if !dir.is_dir() {
        return Err("Trabajo no encontrado".into());
    }
    std::fs::write(dir.join("discarded"), b"1").map_err(err)
}

#[tauri::command]
pub async fn retry_failed(app: AppHandle, state: State<'_, AppState>, job_id: String) -> CmdResult<usize> {
    let m = load_manifest(&state, &job_id)?;
    let journal = Journal::load(&journal_path(&state, &job_id));
    let pending: HashSet<u32> = m
        .ops
        .iter()
        .filter(|o| o.action == OpAction::Copy)
        .filter(|o| !matches!(journal.get(&o.idx), Some(OpResult::Done { .. })))
        .map(|o| o.idx)
        .collect();
    if pending.is_empty() {
        return Err("No hay operaciones fallidas ni pendientes".into());
    }
    copy::cleanup_temp_files(&m.dest_root);
    let n = pending.len();
    spawn_job(app, m, Some(pending))?;
    Ok(n)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportView {
    #[serde(flatten)]
    pub report: JobReport,
    pub kind_label: String,
}

/// Reporte sin la lista completa de entradas (se consulta paginada).
#[tauri::command]
pub async fn get_report(state: State<'_, AppState>, job_id: String) -> CmdResult<ReportView> {
    let mut r = load_report(&state, &job_id)?;
    r.entries.clear();
    r.excluded.truncate(500);
    Ok(ReportView { kind_label: kind_label(&r.kind, r.transfer.mode).into(), report: r })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPage {
    pub total: usize,
    pub entries: Vec<ReportEntry>,
}

#[tauri::command]
pub async fn report_entries(
    state: State<'_, AppState>,
    job_id: String,
    status: Option<String>,
    search: Option<String>,
    offset: usize,
    limit: usize,
) -> CmdResult<EntryPage> {
    let r = load_report(&state, &job_id)?;
    let search = search.unwrap_or_default().to_lowercase();
    let filtered: Vec<ReportEntry> = r
        .entries
        .into_iter()
        .filter(|e| status.as_ref().map(|s| &e.status == s).unwrap_or(true))
        .filter(|e| search.is_empty() || e.source.to_lowercase().contains(&search) || e.dest.to_lowercase().contains(&search))
        .collect();
    let total = filtered.len();
    Ok(EntryPage { total, entries: filtered.into_iter().skip(offset).take(limit.clamp(1, 2000)).collect() })
}

#[tauri::command]
pub async fn export_report(state: State<'_, AppState>, job_id: String, format: String, path: String) -> CmdResult<String> {
    let r = load_report(&state, &job_id)?;
    let content = match format.as_str() {
        "csv" => report::to_csv(&r),
        "json" => report::to_json(&r),
        "txt" => report::to_txt(&r),
        "pathmap" => report::to_path_map(&r),
        _ => return Err("Formato no admitido".into()),
    };
    // La ruta la elige el usuario en el diálogo de guardado del sistema.
    std::fs::write(&path, content).map_err(err)?;
    Ok(path)
}
