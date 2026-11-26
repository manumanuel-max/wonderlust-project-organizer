mod commands;
mod integration;
mod jobs;
mod locations;
mod state;
mod updater;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Una sola instancia: si la app ya está abierta, las carpetas del clic derecho llegan a ella.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            integration::deliver(app, integration::paths_from_args(argv));
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(updater::UpdateState::default())
        .manage(integration::LaunchPaths(std::sync::Mutex::new(integration::paths_from_args(std::env::args()))))
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            std::fs::create_dir_all(&data_dir)?;
            let state = AppState::new(config_dir, data_dir);
            // Trabajos que figuraban "en curso" al cerrarse la aplicación quedan como interrumpidos.
            let hp = state.history_path();
            let mut h = wpo_core::history::load(&hp);
            let mut changed = false;
            for e in h.entries.iter_mut().filter(|e| e.status == "running") {
                e.status = "interrupted".into();
                changed = true;
            }
            if changed {
                let _ = wpo_core::settings::save_json(&hp, &h);
            }
            integration::sync_on_startup(&state);
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::reset_settings,
            commands::preview_project_name,
            commands::parse_project_name,
            commands::check_structure,
            commands::create_structure,
            commands::open_path,
            commands::reveal_path,
            commands::path_info,
            commands::list_locations,
            commands::scan_sources,
            commands::cancel_scan,
            commands::close_session,
            commands::update_plan,
            commands::set_item_overrides,
            commands::set_sequence_decision,
            commands::set_group_enabled,
            commands::query_items,
            commands::tree_children,
            commands::get_plan_details,
            commands::run_analysis,
            commands::cancel_analysis,
            commands::resolve_conflicts,
            jobs::start_job,
            jobs::pause_job,
            jobs::resume_job,
            jobs::cancel_job,
            jobs::job_status,
            jobs::list_interrupted_jobs,
            jobs::resume_interrupted_job,
            jobs::discard_interrupted_job,
            jobs::retry_failed,
            jobs::get_report,
            jobs::report_entries,
            jobs::export_report,
            commands::history_list,
            commands::history_delete,
            updater::update_config,
            updater::check_update,
            updater::install_update,
            updater::restart_app,
            integration::take_launch_paths,
            integration::context_menu_status,
            integration::set_context_menu,
        ])
        .build(tauri::generate_context!())
        .expect("error al iniciar Wonderlust Project Organizer")
        .run(|_app, _event| {
            // macOS: carpetas recibidas desde la Acción rápida de Finder o arrastradas al Dock.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = _event {
                let paths = urls
                    .into_iter()
                    .filter_map(|u| u.to_file_path().ok())
                    .map(|p| p.display().to_string())
                    .collect();
                integration::deliver(_app, paths);
            }
        });
}
