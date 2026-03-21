use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu, CheckMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, State, Emitter,
};

mod config;
mod usage;

use config::AppConfig;
use usage::UsageData;

/// App state shared across commands and tray
struct AppState {
    config: Mutex<AppConfig>,
    usage: Mutex<Option<UsageData>>,
    org_id: Mutex<String>,
    plan: Mutex<String>,
    logged_in: Mutex<bool>,
}

// ──────────────── Tauri Commands (called from JS) ────────────────

/// Check login by calling the usage API from the WebView context.
/// The frontend does the actual fetch and passes the result here.
#[tauri::command]
fn save_login(org_id: String, plan: String, state: State<AppState>) -> Result<(), String> {
    *state.org_id.lock().unwrap() = org_id.clone();
    *state.plan.lock().unwrap() = plan;
    *state.logged_in.lock().unwrap() = true;

    // Save org_id to config dir
    let config = state.config.lock().unwrap();
    config.save_session(&org_id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_config(state: State<AppState>) -> AppConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn update_config(key: String, value: String, state: State<AppState>) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    match key.as_str() {
        "gauge_mode" => config.gauge_mode = value,
        "theme" => config.theme = value,
        "poll_interval_sec" => {
            config.poll_interval_sec = value.parse().unwrap_or(180);
        }
        _ => {}
    }
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

/// Called by frontend when usage data is fetched via WebView fetch()
#[tauri::command]
fn report_usage(data: UsageData, app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    *state.usage.lock().unwrap() = Some(data.clone());

    // Update tray tooltip
    if let Some(tray) = app.tray_by_id("main-tray") {
        let config = state.config.lock().unwrap();
        let plan = state.plan.lock().unwrap();
        let tooltip = usage::format_tooltip(&data, &config.gauge_mode, &plan);
        let _ = tray.set_tooltip(Some(&tooltip));
    }

    // Emit to all windows so the popup can update
    let _ = app.emit("usage-updated", &data);
    Ok(())
}

#[tauri::command]
fn get_usage(state: State<AppState>) -> Option<UsageData> {
    state.usage.lock().unwrap().clone()
}

#[tauri::command]
fn get_session() -> Option<String> {
    AppConfig::load_session()
}

// ──────────────── App Setup ────────────────

pub fn run() {
    let config = AppConfig::load();
    let has_session = AppConfig::load_session().is_some();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            config: Mutex::new(config),
            usage: Mutex::new(None),
            org_id: Mutex::new(String::new()),
            plan: Mutex::new(String::new()),
            logged_in: Mutex::new(false),
        })
        .setup(move |app| {
            // ── System Tray ──
            let quit = MenuItem::with_id(app, "quit", "Quit Claude Tank", true, None::<&str>)?;
            let refresh = MenuItem::with_id(app, "refresh", "Refresh Now", true, None::<&str>)?;
            let dashboard = MenuItem::with_id(app, "dashboard", "Open Dashboard", true, None::<&str>)?;
            let update_key = MenuItem::with_id(app, "update_key", "Update Session", true, None::<&str>)?;
            let about = MenuItem::with_id(app, "about", "Quatrex / Claude Tank v1.0", false, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;

            let menu = Menu::with_items(app, &[
                &dashboard, &refresh, &sep,
                &update_key, &sep2, &about, &quit,
            ])?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .tooltip("Claude Tank — Loading...")
                .icon(app.default_window_icon().cloned().unwrap())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "quit" => app.exit(0),
                        "refresh" => {
                            let _ = app.emit("do-refresh", ());
                        }
                        "dashboard" => {
                            // Show/create the main window
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "update_key" => {
                            let _ = app.emit("do-relogin", ());
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    // Left click → show/hide popup window near tray icon
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let (px, py) = match rect.position {
                                    tauri::Position::Physical(p) => (p.x, p.y),
                                    tauri::Position::Logical(p) => (p.x as i32, p.y as i32),
                                };
                                let _ = w.set_position(tauri::Position::Physical(
                                    tauri::PhysicalPosition { x: px - 180, y: py - 420 },
                                ));
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // ── Create main window (popup dashboard near tray) ──
            let main_url = if has_session {
                // Returning user: show dashboard, JS will auto-poll
                "index.html"
            } else {
                // First time: show login page
                "login.html"
            };

            // The main window is created by tauri.conf.json
            // We'll handle navigation in JS based on session state

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            save_login,
            get_config,
            update_config,
            report_usage,
            get_usage,
            get_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Claude Tank");
}
