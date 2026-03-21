use crate::config::AppConfig;
use std::sync::{Arc, Mutex};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, CheckMenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

// Icon layout — balanced outer margin, tight gap between bars
const ICON_SIZE: u32 = 32;
const BAR_WIDTH: u32 = 12;
const BAR_TOP: u32 = 3;
const BAR_BOTTOM: u32 = 29;
const LEFT_BAR_X: u32 = 3;     // 3px left margin
const RIGHT_BAR_X: u32 = 17;   // gap = 17 - (3+12) = 2px
// outer right margin = 32 - (17+12) = 3px → symmetric

// Cyber theme colors
const BG: [u8; 4] = [0x0A, 0x0A, 0x0F, 0xFF];
const BORDER: [u8; 4] = [0x00, 0xFF, 0xCC, 0x40];
const EMPTY_BAR: [u8; 4] = [0x15, 0x15, 0x20, 0xFF];
const SCANLINE: [u8; 4] = [0x10, 0x10, 0x18, 0xFF];
const BAR_BORDER: [u8; 4] = [0x00, 0xFF, 0xCC, 0x60];

fn cyber_color(remaining_pct: f64) -> [u8; 4] {
    if remaining_pct > 50.0 { [0x00, 0xFF, 0xCC, 0xFF] }       // cyan
    else if remaining_pct > 20.0 { [0xFF, 0xD7, 0x00, 0xFF] }  // gold
    else if remaining_pct > 5.0 { [0xFF, 0x00, 0xFF, 0xFF] }   // magenta
    else { [0xFF, 0x22, 0x44, 0xFF] }                           // neon red
}

fn draw_bar(rgba: &mut [u8], bar_x: u32, fill_pct: f64, color: [u8; 4]) {
    let bar_h = BAR_BOTTOM - BAR_TOP;
    let fill_h = (bar_h as f64 * fill_pct / 100.0).round() as u32;
    let fill_line = BAR_BOTTOM.saturating_sub(fill_h);

    for y in BAR_TOP..BAR_BOTTOM {
        for x in bar_x..bar_x + BAR_WIDTH {
            let idx = ((y * ICON_SIZE + x) * 4) as usize;

            if y == BAR_TOP || y == BAR_BOTTOM - 1 || x == bar_x || x == bar_x + BAR_WIDTH - 1 {
                rgba[idx..idx + 4].copy_from_slice(&BAR_BORDER);
                continue;
            }

            if y >= fill_line {
                rgba[idx..idx + 4].copy_from_slice(&color);
                // Glow center
                if x >= bar_x + 3 && x < bar_x + BAR_WIDTH - 3 {
                    rgba[idx] = rgba[idx].saturating_add(30);
                    rgba[idx + 1] = rgba[idx + 1].saturating_add(30);
                    rgba[idx + 2] = rgba[idx + 2].saturating_add(30);
                }
            } else {
                let c = if y % 3 == 0 { SCANLINE } else { EMPTY_BAR };
                rgba[idx..idx + 4].copy_from_slice(&c);
            }
        }
    }
}

fn generate_icon(five_hour_remaining: f64, seven_day_remaining: f64) -> Icon {
    let mut rgba = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let idx = ((y * ICON_SIZE + x) * 4) as usize;
            if x == 0 || x == ICON_SIZE - 1 || y == 0 || y == ICON_SIZE - 1 {
                rgba[idx..idx + 4].copy_from_slice(&BORDER);
            } else {
                rgba[idx..idx + 4].copy_from_slice(&BG);
            }
        }
    }

    draw_bar(&mut rgba, LEFT_BAR_X, five_hour_remaining, cyber_color(five_hour_remaining));
    draw_bar(&mut rgba, RIGHT_BAR_X, seven_day_remaining, cyber_color(seven_day_remaining));

    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE).expect("Failed to create icon")
}

pub fn create_tray(
    config: Arc<Mutex<AppConfig>>,
    app_tx: std::sync::mpsc::Sender<crate::AppMessage>,
    strings: &crate::i18n::Strings,
) -> TrayIcon {
    let quit = MenuItem::new(strings.get("menu_quit"), true, None);
    let refresh = MenuItem::new(strings.get("menu_refresh"), true, None);
    let about = MenuItem::new("Quatrex / Claude Tank v1.3", false, None);

    let poll_1m = CheckMenuItem::new(strings.get("menu_interval_1m"), true, false, None);
    let poll_3m = CheckMenuItem::new(strings.get("menu_interval_3m"), true, true, None);
    let poll_5m = CheckMenuItem::new(strings.get("menu_interval_5m"), true, false, None);
    let poll_menu = Submenu::with_items(strings.get("menu_interval"), true, &[&poll_1m, &poll_3m, &poll_5m])
        .expect("Failed to create poll submenu");

    let quit_id = quit.id().clone();
    let poll_1m_id = poll_1m.id().clone();
    let poll_3m_id = poll_3m.id().clone();
    let poll_5m_id = poll_5m.id().clone();

    let menu = Menu::with_items(&[
        &refresh,
        &poll_menu,
        &PredefinedMenuItem::separator(),
        &about,
        &quit,
    ]).expect("Failed to create menu");

    let tray = TrayIconBuilder::new()
        .with_tooltip(&format!("Claude Tank\n{}", strings.get("tray_connecting")))
        .with_icon(generate_icon(100.0, 100.0))
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false) // Left click = popup, right click = menu
        .build()
        .expect("Failed to create tray icon");

    // Tray left-click → open popup
    let tx_click = app_tx;
    std::thread::spawn(move || {
        while let Ok(event) = TrayIconEvent::receiver().recv() {
            if let TrayIconEvent::Click { button: tray_icon::MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, position, .. } = event {
                let _ = tx_click.send(crate::AppMessage::TrayClicked {
                    x: position.x as i32,
                    y: position.y as i32,
                });
            }
        }
    });

    // Menu event handler
    let config_for_menu = config;
    std::thread::spawn(move || {
        while let Ok(event) = MenuEvent::receiver().recv() {
            if event.id == quit_id {
                std::process::exit(0);
            } else if event.id == poll_1m_id || event.id == poll_3m_id || event.id == poll_5m_id {
                let secs = if event.id == poll_1m_id { 60 }
                    else if event.id == poll_3m_id { 180 }
                    else { 300 };
                if let Ok(mut c) = config_for_menu.lock() {
                    c.poll_interval_sec = secs;
                    let _ = c.save();
                }
                println!("Poll interval set to {}s", secs);
            }
        }
    });

    tray
}

pub fn update_tray(
    tray: &TrayIcon, five_hour_util: f64, seven_day_util: f64,
    plan: &str, strings: &crate::i18n::Strings,
) {
    let r5 = 100.0 - five_hour_util;
    let r7 = 100.0 - seven_day_util;
    let left = strings.get("tray_left");
    let _ = tray.set_icon(Some(generate_icon(r5, r7)));
    let _ = tray.set_tooltip(Some(&format!(
        "Claude Tank — {}\n5h: {:.0}% {} | 7d: {:.0}% {}", plan, r5, left, r7, left
    )));
}
