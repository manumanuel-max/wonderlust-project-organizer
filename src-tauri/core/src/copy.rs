//! Motor de copia segura.
//!
//! Por cada archivo: lectura en streaming del origen (solo lectura) calculando
//! SHA-256 → escritura en `.<nombre>.wlpart` → `fsync` → comprobación de que el
//! origen no cambió → relectura y verificación SHA-256 → fecha de modificación →
//! renombrado SIN sobrescritura al nombre final → registro en el diario.

use crate::duplicates::{ExcludedEntry, OpAction, Operation};
use crate::error::{CoreError, CoreResult};
use crate::hashing::sha256_file;
use crate::paths::{join_rel, long_path, rename_no_clobber};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, UNIX_EPOCH};

pub const TEMP_EXT: &str = "wlpart";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum VerifyMode {
    /// Relee la copia y compara SHA-256 con el origen (recomendado).
    #[default]
    Full,
    /// Solo compara tamaños (más rápido, menos seguro).
    Size,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct CopySettings {
    pub concurrency: usize,
    pub retries: u32,
    pub buffer_kb: usize,
    pub verify: VerifyMode,
    pub preserve_mtime: bool,
}

impl Default for CopySettings {
    fn default() -> Self {
        CopySettings { concurrency: 2, retries: 2, buffer_kb: 4096, verify: VerifyMode::Full, preserve_mtime: true }
    }
}

/// Copiar (los originales quedan intactos) o mover (los originales se retiran
/// solo después de verificar la copia).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TransferMode {
    #[default]
    Copy,
    Move,
}

/// Qué hacer con el original al mover entre discos distintos, tras verificar la copia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RemovalMode {
    /// Enviar a la Papelera / Papelera de reciclaje (recuperable).
    #[default]
    Trash,
    /// Eliminar definitivamente.
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct TransferOptions {
    pub mode: TransferMode,
    pub removal: RemovalMode,
}

/// Estado del archivo original tras la operación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OriginState {
    /// El original sigue en su sitio (modo copiar).
    #[default]
    Kept,
    /// Movido dentro del mismo disco (renombrado atómico; el contenido no se reescribe).
    Moved,
    /// Copiado, verificado y el original enviado a la Papelera.
    Trashed,
    /// Copiado, verificado y el original eliminado.
    Deleted,
    /// Copiado y verificado, pero el original no se pudo retirar (sigue en su sitio).
    RemovalFailed { error: String },
}

/// Descripción persistente de un trabajo (permite reanudar tras un cierre).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobManifest {
    pub job_id: String,
    pub created_at: String,
    pub kind: String,
    pub client: String,
    pub project: String,
    pub dest_root: PathBuf,
    pub sources: Vec<PathBuf>,
    pub ops: Vec<Operation>,
    pub dirs: Vec<String>,
    pub excluded: Vec<ExcludedEntry>,
    pub settings: CopySettings,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub transfer: TransferOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum OpResult {
    Pending,
    Done {
        sha256: String,
        verified: bool,
        #[serde(default)]
        origin: OriginState,
    },
    Skipped { reason: String },
    Failed { error: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct JournalLine {
    idx: u32,
    #[serde(flatten)]
    result: OpResult,
}

/// Diario append-only (JSONL). Tolera una última línea incompleta.
pub struct Journal {
    file: Mutex<File>,
    unsynced: AtomicUsize,
}

impl Journal {
    pub fn load(path: &Path) -> HashMap<u32, OpResult> {
        let mut out = HashMap::new();
        if let Ok(f) = File::open(path) {
            for line in BufReader::new(f).lines().map_while(Result::ok) {
                if let Ok(l) = serde_json::from_str::<JournalLine>(&line) {
                    out.insert(l.idx, l.result);
                }
            }
        }
        out
    }

    pub fn open(path: &Path) -> CoreResult<Journal> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| CoreError::io(p, e))?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path).map_err(|e| CoreError::io(path, e))?;
        Ok(Journal { file: Mutex::new(file), unsynced: AtomicUsize::new(0) })
    }

    pub fn record(&self, idx: u32, result: &OpResult) {
        let line = serde_json::to_string(&JournalLine { idx, result: result.clone() }).unwrap_or_default();
        let mut f = self.file.lock().unwrap();
        let _ = writeln!(f, "{line}");
        let _ = f.flush();
        if self.unsynced.fetch_add(1, Ordering::Relaxed) >= 25 {
            let _ = f.sync_data();
            self.unsynced.store(0, Ordering::Relaxed);
        }
    }

    pub fn sync(&self) {
        let _ = self.file.lock().unwrap().sync_data();
    }
}

#[derive(Debug, Default)]
pub struct JobControl {
    pub pause: AtomicBool,
    pub cancel: AtomicBool,
    abort_reason: Mutex<Option<String>>,
}

impl JobControl {
    pub fn abort(&self, reason: String) {
        let mut r = self.abort_reason.lock().unwrap();
        if r.is_none() {
            *r = Some(reason);
        }
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn abort_reason(&self) -> Option<String> {
        self.abort_reason.lock().unwrap().clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveFile {
    pub idx: u32,
    pub name: String,
    pub done: u64,
    pub total: u64,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub files_done: usize,
    pub files_total: usize,
    pub files_failed: usize,
    pub files_skipped: usize,
    pub speed_bps: f64,
    pub elapsed_ms: u64,
    pub eta_ms: Option<u64>,
    pub active: Vec<ActiveFile>,
    pub paused: bool,
    pub finished: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobOutcome {
    pub results: Vec<OpResult>,
    pub cancelled: bool,
    pub aborted_reason: Option<String>,
    pub duration_ms: u64,
    pub bytes_copied: u64,
    /// Carpetas vacías retiradas del origen (solo al mover).
    #[serde(default)]
    pub pruned_dirs: usize,
}

enum CopyError {
    Cancelled,
    Fatal(String),
    Retryable(String),
    NoSpace(String),
}

struct Shared<'a> {
    bytes: AtomicU64,
    done: AtomicUsize,
    failed: AtomicUsize,
    active: Mutex<HashMap<u32, ActiveFile>>,
    control: &'a JobControl,
}

fn io_err(context: &str, e: io::Error) -> CopyError {
    if e.kind() == io::ErrorKind::StorageFull || e.raw_os_error() == Some(28) || e.raw_os_error() == Some(112) {
        return CopyError::NoSpace(format!("Sin espacio en el destino ({context})"));
    }
    if e.kind() == io::ErrorKind::PermissionDenied {
        return CopyError::Fatal(format!("{context}: permiso denegado"));
    }
    if e.kind() == io::ErrorKind::NotFound {
        return CopyError::Fatal(format!("{context}: no encontrado"));
    }
    CopyError::Retryable(format!("{context}: {e}"))
}

fn temp_path_for(dest: &Path) -> PathBuf {
    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    dest.with_file_name(format!(".{name}.{TEMP_EXT}"))
}

fn wait_if_paused(control: &JobControl) -> Result<(), CopyError> {
    while control.pause.load(Ordering::Relaxed) {
        if control.cancel.load(Ordering::Relaxed) {
            return Err(CopyError::Cancelled);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if control.cancel.load(Ordering::Relaxed) {
        return Err(CopyError::Cancelled);
    }
    Ok(())
}

fn meta_signature(p: &Path) -> io::Result<(u64, Option<std::time::SystemTime>)> {
    let m = std::fs::metadata(long_path(p))?;
    Ok((m.len(), m.modified().ok()))
}

fn copy_one(op: &Operation, dest_root: &Path, settings: &CopySettings, shared: &Shared, counted: &mut u64) -> Result<OpResult, CopyError> {
    let dest = join_rel(dest_root, &op.dest_rel);
    if std::fs::symlink_metadata(long_path(&dest)).is_ok() {
        return Err(CopyError::Fatal("El archivo de destino ya existe; no se sobrescribe".into()));
    }
    let parent = dest.parent().unwrap_or(dest_root);
    std::fs::create_dir_all(long_path(parent)).map_err(|e| io_err("Crear carpeta de destino", e))?;
    let tmp = temp_path_for(&dest);
    // Un temporal previo solo puede ser nuestro (extensión .wlpart): se descarta.
    if tmp.exists() {
        let _ = std::fs::remove_file(long_path(&tmp));
    }
    let before = meta_signature(&op.src).map_err(|e| io_err("Leer origen", e))?;

    let result = (|| -> Result<String, CopyError> {
        let mut src = File::open(long_path(&op.src)).map_err(|e| io_err("Abrir origen", e))?;
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(long_path(&tmp))
            .map_err(|e| io_err("Crear archivo temporal", e))?;
        let mut buf = vec![0u8; settings.buffer_kb.clamp(64, 65536) * 1024];
        let mut hasher = Sha256::new();
        let mut written: u64 = 0;
        loop {
            wait_if_paused(shared.control)?;
            let n = match src.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_err("Leer origen", e)),
            };
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|e| io_err("Escribir destino", e))?;
            written += n as u64;
            *counted += n as u64;
            shared.bytes.fetch_add(n as u64, Ordering::Relaxed);
            if let Some(a) = shared.active.lock().unwrap().get_mut(&op.idx) {
                a.done = written;
            }
        }
        out.flush().map_err(|e| io_err("Escribir destino", e))?;
        out.sync_all().map_err(|e| io_err("Sincronizar destino", e))?;
        drop(out);
        let src_hash = hex::encode(hasher.finalize());

        let after = meta_signature(&op.src).map_err(|e| io_err("Leer origen", e))?;
        if after != before || written != before.0 {
            return Err(CopyError::Retryable("El archivo de origen cambió durante la copia".into()));
        }
        let dest_len = std::fs::metadata(long_path(&tmp)).map_err(|e| io_err("Verificar destino", e))?.len();
        if dest_len != written {
            return Err(CopyError::Retryable("El tamaño de la copia no coincide".into()));
        }
        if settings.verify == VerifyMode::Full {
            if let Some(a) = shared.active.lock().unwrap().get_mut(&op.idx) {
                a.phase = "Verificando".into();
            }
            let dest_hash = sha256_file(&tmp, &shared.control.cancel, |_| {}).map_err(|e| {
                if e.kind() == io::ErrorKind::Interrupted {
                    CopyError::Cancelled
                } else {
                    io_err("Verificar destino", e)
                }
            })?;
            if dest_hash != src_hash {
                return Err(CopyError::Retryable("La verificación SHA-256 no coincide".into()));
            }
        }
        if settings.preserve_mtime && op.mtime > 0 {
            let _ = filetime::set_file_mtime(long_path(&tmp), filetime::FileTime::from_unix_time(op.mtime, 0));
        }
        Ok(src_hash)
    })();

    match result {
        Ok(hash) => match rename_no_clobber(&long_path(&tmp), &long_path(&dest)) {
            Ok(()) => Ok(OpResult::Done { sha256: hash, verified: settings.verify == VerifyMode::Full, origin: OriginState::Kept }),
            Err(e) => {
                let _ = std::fs::remove_file(long_path(&tmp));
                if e.kind() == io::ErrorKind::AlreadyExists {
                    Err(CopyError::Fatal("El archivo de destino apareció durante la copia; no se sobrescribe".into()))
                } else {
                    Err(io_err("Renombrar copia", e))
                }
            }
        },
        Err(e) => {
            let _ = std::fs::remove_file(long_path(&tmp));
            Err(e)
        }
    }
}

/// Si el destino ya existe sin registro en el diario (p. ej. corte de luz
/// justo tras el renombrado), se acepta solo si el contenido es idéntico.
fn reconcile_existing(op: &Operation, dest: &Path, cancel: &AtomicBool) -> Option<OpResult> {
    let m = std::fs::metadata(long_path(dest)).ok()?;
    if !m.is_file() || m.len() != op.size {
        return None;
    }
    let a = sha256_file(&op.src, cancel, |_| {}).ok()?;
    let b = sha256_file(dest, cancel, |_| {}).ok()?;
    (a == b).then_some(OpResult::Done { sha256: a, verified: true, origin: OriginState::Kept })
}

/// Retira el original tras una copia verificada. Antes comprueba que el
/// original no cambió (mismo tamaño) desde que se copió.
pub fn remove_original(op: &Operation, removal: RemovalMode) -> OriginState {
    match std::fs::metadata(long_path(&op.src)) {
        Ok(m) if m.len() == op.size => {}
        Ok(_) => {
            return OriginState::RemovalFailed { error: "El original cambió después de copiarse; no se retiró".into() }
        }
        Err(e) => return OriginState::RemovalFailed { error: e.to_string() },
    }
    match removal {
        RemovalMode::Trash => match trash::delete(&op.src) {
            Ok(()) => OriginState::Trashed,
            Err(e) => OriginState::RemovalFailed { error: format!("No se pudo enviar a la Papelera: {e}") },
        },
        RemovalMode::Delete => match std::fs::remove_file(long_path(&op.src)) {
            Ok(()) => OriginState::Deleted,
            Err(e) => OriginState::RemovalFailed { error: e.to_string() },
        },
    }
}

/// Intenta mover en el mismo disco (renombrado sin sobrescritura).
/// `Ok(None)` significa que no es posible (otro disco, permisos...) y hay que copiar.
fn try_fast_move(op: &Operation, dest: &Path) -> Result<Option<OpResult>, CopyError> {
    if std::fs::symlink_metadata(long_path(dest)).is_ok() {
        return Err(CopyError::Fatal("El archivo de destino ya existe; no se sobrescribe".into()));
    }
    let parent = dest.parent().unwrap_or(dest);
    std::fs::create_dir_all(long_path(parent)).map_err(|e| io_err("Crear carpeta de destino", e))?;
    match std::fs::metadata(long_path(&op.src)) {
        Ok(m) if m.len() == op.size => {}
        Ok(_) => return Err(CopyError::Retryable("El archivo de origen cambió desde el análisis".into())),
        Err(e) => return Err(io_err("Leer origen", e)),
    }
    match rename_no_clobber(&long_path(&op.src), &long_path(dest)) {
        Ok(()) => {
            let len = std::fs::metadata(long_path(dest)).map(|m| m.len()).unwrap_or(u64::MAX);
            if len != op.size {
                return Err(CopyError::Fatal("El tamaño tras mover no coincide".into()));
            }
            Ok(Some(OpResult::Done { sha256: String::new(), verified: true, origin: OriginState::Moved }))
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            Err(CopyError::Fatal("El archivo de destino apareció durante la operación; no se sobrescribe".into()))
        }
        // Otro disco (EXDEV / ERROR_NOT_SAME_DEVICE) u otra limitación: copiar y verificar.
        Err(_) => Ok(None),
    }
}

/// Elimina carpetas que quedaron vacías dentro de los orígenes (de abajo arriba).
/// Nunca elimina la carpeta seleccionada ni carpetas con contenido.
pub fn prune_empty_dirs(sources: &[PathBuf]) -> usize {
    fn walk(dir: &Path, removed: &mut usize) -> bool {
        let Ok(rd) = std::fs::read_dir(dir) else { return false };
        let mut empty = true;
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else {
                empty = false;
                continue;
            };
            if ft.is_dir() {
                if walk(&e.path(), removed) {
                    if std::fs::remove_dir(e.path()).is_ok() {
                        *removed += 1;
                    } else {
                        empty = false;
                    }
                } else {
                    empty = false;
                }
            } else {
                empty = false;
            }
        }
        empty
    }
    let mut removed = 0;
    for s in sources.iter().filter(|s| s.is_dir()) {
        walk(s, &mut removed);
    }
    removed
}

/// Ejecuta (o reanuda) un trabajo. `only` limita la ejecución a ciertos índices
/// (reintentar fallidos).
pub fn run_job(
    manifest: &JobManifest,
    journal_path: &Path,
    control: &JobControl,
    only: Option<&HashSet<u32>>,
    on_progress: impl Fn(&Progress) + Sync,
) -> CoreResult<JobOutcome> {
    let started = Instant::now();
    let mut move_settings = manifest.settings.clone();
    move_settings.verify = VerifyMode::Full;
    let previous = Journal::load(journal_path);
    let journal = Journal::open(journal_path)?;
    let mut results: Vec<OpResult> = manifest
        .ops
        .iter()
        .map(|o| previous.get(&o.idx).cloned().unwrap_or(OpResult::Pending))
        .collect();

    std::fs::create_dir_all(long_path(&manifest.dest_root)).map_err(|e| CoreError::io(&manifest.dest_root, e))?;
    for d in &manifest.dirs {
        let p = join_rel(&manifest.dest_root, d);
        std::fs::create_dir_all(long_path(&p)).map_err(|e| CoreError::io(&p, e))?;
    }

    // Trabajo pendiente.
    let mut work: Vec<&Operation> = Vec::new();
    for (i, op) in manifest.ops.iter().enumerate() {
        if let Some(set) = only {
            if !set.contains(&op.idx) {
                continue;
            }
        }
        match (&op.action, &results[i]) {
            (OpAction::Skip { reason }, OpResult::Pending) => {
                let r = OpResult::Skipped { reason: reason.clone() };
                journal.record(op.idx, &r);
                results[i] = r;
            }
            (OpAction::Copy, OpResult::Done { .. }) => {
                // Ya verificado en una ejecución anterior: comprobar que sigue ahí.
                let dest = join_rel(&manifest.dest_root, &op.dest_rel);
                let ok = std::fs::metadata(long_path(&dest)).map(|m| m.len() == op.size).unwrap_or(false);
                if !ok {
                    results[i] = OpResult::Pending;
                    work.push(op);
                }
            }
            (OpAction::Copy, OpResult::Pending) | (OpAction::Copy, OpResult::Failed { .. }) => work.push(op),
            _ => {}
        }
    }

    let files_total = manifest.ops.iter().filter(|o| o.action == OpAction::Copy).count();
    let bytes_total: u64 = manifest.ops.iter().filter(|o| o.action == OpAction::Copy).map(|o| o.size).sum();
    let already_bytes: u64 = manifest
        .ops
        .iter()
        .zip(&results)
        .filter(|(o, r)| o.action == OpAction::Copy && matches!(r, OpResult::Done { .. }) && !work.iter().any(|w| w.idx == o.idx))
        .map(|(o, _)| o.size)
        .sum();
    let already_done = results.iter().filter(|r| matches!(r, OpResult::Done { .. })).count();
    let skipped = results.iter().filter(|r| matches!(r, OpResult::Skipped { .. })).count();

    let shared = Shared {
        bytes: AtomicU64::new(already_bytes),
        done: AtomicUsize::new(already_done),
        failed: AtomicUsize::new(0),
        active: Mutex::new(HashMap::new()),
        control,
    };
    let next = AtomicUsize::new(0);
    let new_results: Mutex<Vec<(u32, OpResult)>> = Mutex::new(Vec::new());
    let finished = AtomicBool::new(false);
    let copied_now = AtomicU64::new(0);
    let threads = manifest.settings.concurrency.clamp(1, 16).min(work.len().max(1));
    let idx_pos: HashMap<u32, usize> = manifest.ops.iter().enumerate().map(|(i, o)| (o.idx, i)).collect();

    let make_progress = |samples: &VecDeque<(Instant, u64)>, finished_flag: bool| -> Progress {
        let bytes_done = shared.bytes.load(Ordering::Relaxed);
        let speed = match (samples.front(), samples.back()) {
            (Some(a), Some(b)) if b.0 > a.0 => (b.1.saturating_sub(a.1)) as f64 / (b.0 - a.0).as_secs_f64(),
            _ => 0.0,
        };
        let remaining = bytes_total.saturating_sub(bytes_done);
        let mut active: Vec<ActiveFile> = shared.active.lock().unwrap().values().cloned().collect();
        active.sort_by_key(|a| a.idx);
        Progress {
            bytes_done,
            bytes_total,
            files_done: shared.done.load(Ordering::Relaxed),
            files_total,
            files_failed: shared.failed.load(Ordering::Relaxed),
            files_skipped: skipped,
            speed_bps: speed,
            elapsed_ms: started.elapsed().as_millis() as u64,
            eta_ms: (speed > 1.0).then(|| (remaining as f64 / speed * 1000.0) as u64),
            active,
            paused: control.pause.load(Ordering::Relaxed),
            finished: finished_flag,
        }
    };

    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                if control.cancel.load(Ordering::Relaxed) {
                    break;
                }
                let n = next.fetch_add(1, Ordering::SeqCst);
                let Some(op) = work.get(n) else { break };
                let dest = join_rel(&manifest.dest_root, &op.dest_rel);
                let moving = manifest.transfer.mode == TransferMode::Move;
                if !previous.contains_key(&op.idx) && dest.exists() {
                    // Cierre inesperado justo después de mover: el destino existe y el original ya no.
                    let moved_already = moving
                        && !op.src.exists()
                        && std::fs::metadata(long_path(&dest)).map(|m| m.len() == op.size).unwrap_or(false);
                    let reconciled = if moved_already {
                        Some(OpResult::Done { sha256: String::new(), verified: true, origin: OriginState::Moved })
                    } else {
                        reconcile_existing(op, &dest, &control.cancel).map(|r| match r {
                            OpResult::Done { sha256, verified, .. } if moving => OpResult::Done {
                                sha256,
                                verified,
                                origin: remove_original(op, manifest.transfer.removal),
                            },
                            other => other,
                        })
                    };
                    if let Some(r) = reconciled {
                        shared.bytes.fetch_add(op.size, Ordering::Relaxed);
                        shared.done.fetch_add(1, Ordering::Relaxed);
                        journal.record(op.idx, &r);
                        new_results.lock().unwrap().push((op.idx, r));
                        continue;
                    }
                }
                shared.active.lock().unwrap().insert(
                    op.idx,
                    ActiveFile {
                        idx: op.idx,
                        name: op.dest_rel.rsplit('/').next().unwrap_or("").to_string(),
                        done: 0,
                        total: op.size,
                        phase: if moving { "Moviendo".into() } else { "Copiando".into() },
                    },
                );
                let mut attempt = 0;
                let outcome = loop {
                    let mut counted = 0u64;
                    let r = if moving {
                        match try_fast_move(op, &dest) {
                            Ok(Some(res)) => {
                                counted = op.size;
                                shared.bytes.fetch_add(op.size, Ordering::Relaxed);
                                Ok(res)
                            }
                            Ok(None) => copy_one(op, &manifest.dest_root, &move_settings, &shared, &mut counted).map(|res| match res {
                                OpResult::Done { sha256, verified, .. } => {
                                    if let Some(a) = shared.active.lock().unwrap().get_mut(&op.idx) {
                                        a.phase = "Retirando original".into();
                                    }
                                    OpResult::Done { sha256, verified, origin: remove_original(op, manifest.transfer.removal) }
                                }
                                other => other,
                            }),
                            Err(e) => Err(e),
                        }
                    } else {
                        copy_one(op, &manifest.dest_root, &manifest.settings, &shared, &mut counted)
                    };
                    if r.is_err() {
                        shared.bytes.fetch_sub(counted, Ordering::Relaxed);
                    } else {
                        copied_now.fetch_add(counted, Ordering::Relaxed);
                    }
                    match r {
                        Ok(res) => break Some(res),
                        Err(CopyError::Cancelled) => break None,
                        Err(CopyError::NoSpace(m)) => {
                            control.abort(m.clone());
                            break Some(OpResult::Failed { error: m });
                        }
                        Err(CopyError::Fatal(m)) => break Some(OpResult::Failed { error: m }),
                        Err(CopyError::Retryable(m)) => {
                            if attempt >= manifest.settings.retries {
                                break Some(OpResult::Failed { error: format!("{m} (tras {} intento(s))", attempt + 1) });
                            }
                            attempt += 1;
                            if let Some(a) = shared.active.lock().unwrap().get_mut(&op.idx) {
                                a.phase = format!("Reintento {attempt}");
                                a.done = 0;
                            }
                            std::thread::sleep(Duration::from_millis(300 * attempt as u64));
                        }
                    }
                };
                shared.active.lock().unwrap().remove(&op.idx);
                if let Some(res) = outcome {
                    match &res {
                        OpResult::Done { .. } => {
                            shared.done.fetch_add(1, Ordering::Relaxed);
                        }
                        OpResult::Failed { .. } => {
                            shared.failed.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => {}
                    }
                    journal.record(op.idx, &res);
                    new_results.lock().unwrap().push((op.idx, res));
                }
            });
        }
        // Hilo informador (este mismo hilo).
        let mut samples: VecDeque<(Instant, u64)> = VecDeque::new();
        loop {
            let all_done = next.load(Ordering::SeqCst) >= work.len() + threads && shared.active.lock().unwrap().is_empty();
            let now = Instant::now();
            samples.push_back((now, shared.bytes.load(Ordering::Relaxed)));
            while samples.len() > 2 && now.duration_since(samples.front().unwrap().0) > Duration::from_secs(5) {
                samples.pop_front();
            }
            if all_done || (control.cancel.load(Ordering::Relaxed) && shared.active.lock().unwrap().is_empty()) {
                break;
            }
            on_progress(&make_progress(&samples, false));
            std::thread::sleep(Duration::from_millis(200));
        }
    });
    finished.store(true, Ordering::SeqCst);
    journal.sync();

    for (idx, r) in new_results.into_inner().unwrap() {
        if let Some(pos) = idx_pos.get(&idx) {
            results[*pos] = r;
        }
    }
    let progress_final = {
        let samples = VecDeque::new();
        let mut p = make_progress(&samples, true);
        p.active.clear();
        p
    };
    on_progress(&progress_final);
    let cancelled = control.cancel.load(Ordering::Relaxed);
    let pruned_dirs = if manifest.transfer.mode == TransferMode::Move && !cancelled {
        prune_empty_dirs(&manifest.sources)
    } else {
        0
    };
    Ok(JobOutcome {
        pruned_dirs,
        results,
        cancelled: cancelled && control.abort_reason().is_none(),
        aborted_reason: control.abort_reason(),
        duration_ms: started.elapsed().as_millis() as u64,
        bytes_copied: copied_now.load(Ordering::Relaxed),
    })
}

/// Elimina temporales `.wlpart` que hayan quedado en el destino tras un cierre
/// inesperado. Solo toca archivos con nuestra extensión dentro del destino.
pub fn cleanup_temp_files(dest_root: &Path) -> usize {
    let mut removed = 0;
    let mut stack = vec![dest_root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                stack.push(p);
            } else if ft.is_file() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') && name.ends_with(&format!(".{TEMP_EXT}")) && std::fs::remove_file(&p).is_ok() {
                    removed += 1;
                }
            }
        }
    }
    removed
}

pub fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}
