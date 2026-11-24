//! AgentDock Tauri backend library entry point, native macOS menu setup, and IPC registration.

pub mod adapters;
pub mod backup;
pub mod commands;
pub mod config;
pub mod debloat;
pub mod error;
pub mod frontmatter;
pub mod git_hygiene;
pub mod mcp_catalog;
pub mod mcp_probe;
pub mod mcp_writer;
pub mod models;
pub mod rules_writer;
pub mod scanner;
pub mod support_docs;
pub mod symlinks;
pub mod tokenizer;
pub mod watcher;

use std::sync::Mutex;

use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};

use crate::watcher::WatcherManager;

/// Builds and runs the Tauri v2 application.
///
/// # Panics
/// Panics only if Tauri fails to initialize the native application runtime.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(WatcherManager::new()))
        .setup(|app| {
            let menu = build_app_menu(app)?;
            app.set_menu(menu)?;

            if let (Ok(cfg), Ok(home)) = (config::load_config(), config::home_dir()) {
                let state = app.state::<Mutex<WatcherManager>>();
                if let Ok(mut mgr) = state.lock() {
                    mgr.refresh_watches(app.handle(), &cfg, &home, None);
                };
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            let payload_event = match event.id().as_ref() {
                "app-settings" => Some("menu://settings"),
                "refresh-workspace" => Some("menu://refresh"),
                "quick-filter" => Some("menu://search"),
                _ => None,
            };
            if let Some(name) = payload_event {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.emit(name, ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::scan_workspace,
            commands::scan_project,
            commands::set_watched_project,
            commands::reveal_in_finder,
            commands::open_in_default_app,
            commands::toggle_skill,
            commands::unlink_broken_symlink,
            commands::toggle_library_rule,
            commands::preview_move_skills_to_library,
            commands::execute_move_skills_to_library,
            commands::update_rule_trigger,
            commands::link_support_doc,
            commands::unlink_support_doc,
            commands::toggle_mcp_server,
            commands::add_mcp_server,
            commands::probe_mcp_server,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AgentDock Tauri application");
}

fn build_app_menu<R: tauri::Runtime>(
    app: &tauri::App<R>,
) -> Result<tauri::menu::Menu<R>, tauri::Error> {
    let settings_item = MenuItemBuilder::with_id("app-settings", "Settings…")
        .accelerator("CmdOrCtrl+,")
        .build(app)?;
    let refresh_item = MenuItemBuilder::with_id("refresh-workspace", "Refresh Workspace")
        .accelerator("CmdOrCtrl+R")
        .build(app)?;
    let filter_item = MenuItemBuilder::with_id("quick-filter", "Quick Filter")
        .accelerator("CmdOrCtrl+K")
        .build(app)?;

    #[cfg(target_os = "macos")]
    let app_submenu = SubmenuBuilder::new(app, "AgentDock")
        .about(None)
        .separator()
        .item(&settings_item)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file_submenu = SubmenuBuilder::new(app, "File")
        .item(&refresh_item)
        .separator()
        .close_window()
        .build()?;

    let edit_submenu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view_submenu = SubmenuBuilder::new(app, "View")
        .item(&filter_item)
        .separator()
        .fullscreen()
        .build()?;

    let window_submenu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .close_window()
        .build()?;

    #[cfg(target_os = "macos")]
    return MenuBuilder::new(app)
        .items(&[
            &app_submenu,
            &file_submenu,
            &edit_submenu,
            &view_submenu,
            &window_submenu,
        ])
        .build();

    #[cfg(not(target_os = "macos"))]
    return MenuBuilder::new(app)
        .items(&[&file_submenu, &edit_submenu, &view_submenu, &window_submenu])
        .build();
}
