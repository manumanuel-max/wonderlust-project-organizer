use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use wpo_core::copy::{JobControl, Progress};
use wpo_core::duplicates::{Analysis, Resolution};
use wpo_core::plan::{Overrides, Plan, PlanOptions};
use wpo_core::preflight::PreflightReport;
use wpo_core::scan::Inventory;
use wpo_core::settings::AppSettings;

/// Sesión de organización en curso (inventario + plan en memoria de Rust;
/// la interfaz lo consulta paginado).
pub struct Session {
    pub id: String,
    pub inventory: Inventory,
    pub options: PlanOptions,
    pub overrides: Overrides,
    pub plan: Plan,
    pub analysis: Option<Analysis>,
    pub preflight: Option<PreflightReport>,
    pub dest_root: Option<PathBuf>,
    pub resolutions: HashMap<u32, Resolution>,
}

pub struct RunningJob {
    pub control: Arc<JobControl>,
    pub last_progress: Arc<Mutex<Option<Progress>>>,
}

pub struct AppState {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub settings: Mutex<AppSettings>,
    pub session: Mutex<Option<Session>>,
    pub scan_cancel: Arc<AtomicBool>,
    pub analysis_cancel: Arc<AtomicBool>,
    pub jobs: Mutex<HashMap<String, RunningJob>>,
}

impl AppState {
    pub fn new(config_dir: PathBuf, data_dir: PathBuf) -> Self {
        let settings = wpo_core::settings::load(&config_dir.join("settings.json"));
        AppState {
            config_dir,
            data_dir,
            settings: Mutex::new(settings),
            session: Mutex::new(None),
            scan_cancel: Arc::new(AtomicBool::new(false)),
            analysis_cancel: Arc::new(AtomicBool::new(false)),
            jobs: Mutex::new(HashMap::new()),
        }
    }

    pub fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
    pub fn history_path(&self) -> PathBuf {
        self.data_dir.join("history.json")
    }
    pub fn jobs_dir(&self) -> PathBuf {
        self.data_dir.join("jobs")
    }
    pub fn job_dir(&self, id: &str) -> PathBuf {
        self.jobs_dir().join(id)
    }
}

/// Convierte cualquier error en un mensaje legible para la interfaz.
pub fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
