pub mod ai;
pub mod backup;
pub mod commands;
pub mod deployment;
pub mod error;
pub mod files;
pub mod monitoring;
pub mod security;
pub mod services;
pub mod ssh;
pub mod state;
pub mod storage;
pub mod terminal;
pub mod website;

use commands::{assistant::*, connections::*, data::*, deploy::*, files_term::*, ops::*};
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = storage::Db::open(&dir.join("vps-manager.sqlite3")).map_err(|e| e.to_string())?;
            if let Ok(s) = storage::settings::load(&db) {
                let _ = storage::logs::purge_older_than(&db, s.log_retention_days);
            }
            app.manage(Arc::new(state::AppState::new(db, dir)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            save_profile,
            delete_profile,
            test_connection,
            connect_profile,
            disconnect_profile,
            connection_states,
            check_connection,
            list_known_hosts,
            forget_known_host,
            get_metrics,
            services_overview,
            list_processes,
            service_logs,
            service_action,
            websites_list,
            website_check_url,
            website_cert_info,
            nginx_test,
            nginx_reload,
            files_list,
            files_mkdir,
            files_rename,
            files_delete,
            files_read_text,
            files_write_text,
            files_download,
            files_upload,
            terminal_open,
            terminal_write,
            terminal_resize,
            terminal_close,
            db_list,
            backup_default_dir,
            backup_create,
            backup_history,
            backup_delete_record,
            backup_remote_list,
            backup_download,
            backup_restore,
            deploy_projects,
            deploy_project_save,
            deploy_project_delete,
            deploy_git_status,
            deploy_history,
            deploy_run,
            deploy_rollback,
            ai_status,
            ai_chat,
            ai_sanitize,
            ai_run_command,
            logs_list,
            logs_clear,
            settings_get,
            settings_set,
        ])
        .run(tauri::generate_context!())
        .expect("gagal menjalankan VPS Manager");
}
