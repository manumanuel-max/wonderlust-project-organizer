mod commands;
mod jobs;
mod locations;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar Wonderlust Project Organizer");
}
