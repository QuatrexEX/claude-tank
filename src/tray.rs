use crate::config::AppConfig;
use std::sync::{Arc, Mutex};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

/// Cyber theme color: remaining% → color
/// High = cyan glow, mid = yellow, low = magenta/red
fn cyber_color(remaining_pct: f64) -> [u8; 4] {
    if remaining_pct > 50.0 { [0x00, 0xFF, 0xCC, 0xFF] }       // cyan #00ffcc
    else if remaining_pct > 20.0 { [0xFF, 0xD7, 0x00, 0xFF] }  // gold #ffd700
    else if remaining_pct > 5.0 { [0xFF, 0x00, 0xFF, 0xFF] }   // magenta #ff00ff
    else { [0xFF, 0x22, 0x44, 0xFF] }                           // neon red #ff2244
}

/// 32x32 cyber-themed dual gauge icon
/// Dark background, neon bars with glow effect, thin cyan border
fn generate_cyber_icon(five_hour_remaining: f64, seven_day_remaining: f64) -> Icon {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    let color_5h = cyber_color(five_hour_remaining);
    let color_7d = cyber_color(seven_day_remaining);

    // Dark background
    let bg = [0x0A, 0x0A, 0x0F, 0xFF];
    let border = [0x00, 0xFF, 0xCC, 0x40]; // cyan semi-transparent
    let empty_bar = [0x15, 0x15, 0x20, 0xFF]; // dark indigo

    // Bar layout
    let bar_w = 11u32;
    let bar_top = 3u32;
    let bar_bottom = 29u32;
    let bar_h = bar_bottom - bar_top;
    let left_x = 2u32;
    let right_x = 19u32;
    let gap_x = 15u32;

    let fill_5h = (bar_h as f64 * five_hour_remaining / 100.0).round() as u32;
    let fill_7d = (bar_h as f64 * seven_day_remaining / 100.0).round() as u32;

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;

            // Background
            rgba[idx..idx+4].copy_from_slice(&bg);

            // Outer border (1px cyan glow)
            if x == 0 || x == size - 1 || y == 0 || y == size - 1 {
                rgba[idx..idx+4].copy_from_slice(&border);
            }

            let in_left = x >= left_x && x < left_x + bar_w && y >= bar_top && y < bar_bottom;
            let in_right = x >= right_x && x < right_x + bar_w && y >= bar_top && y < bar_bottom;

            // 5h bar (left)
            if in_left {
                let fill_line = bar_bottom.saturating_sub(fill_5h);
                if y >= fill_line {
                    rgba[idx..idx+4].copy_from_slice(&color_5h);
                    // Glow: brighter center column
                    if x >= left_x + 3 && x < left_x + bar_w - 3 {
                        rgba[idx] = rgba[idx].saturating_add(30);
                        rgba[idx+1] = rgba[idx+1].saturating_add(30);
                        rgba[idx+2] = rgba[idx+2].saturating_add(30);
                    }
                } else {
                    rgba[idx..idx+4].copy_from_slice(&empty_bar);
                    // Scanline effect on empty area
                    if y % 3 == 0 {
                        rgba[idx..idx+4].copy_from_slice(&[0x10, 0x10, 0x18, 0xFF]);
                    }
                }
                // Bar border
                if y == bar_top || y == bar_bottom - 1 || x == left_x || x == left_x + bar_w - 1 {
                    rgba[idx..idx+4].copy_from_slice(&[0x00, 0xFF, 0xCC, 0x60]);
                }
            }

            // 7d bar (right)
            if in_right {
                let fill_line = bar_bottom.saturating_sub(fill_7d);
                if y >= fill_line {
                    rgba[idx..idx+4].copy_from_slice(&color_7d);
                    if x >= right_x + 3 && x < right_x + bar_w - 3 {
                        rgba[idx] = rgba[idx].saturating_add(30);
                        rgba[idx+1] = rgba[idx+1].saturating_add(30);
                        rgba[idx+2] = rgba[idx+2].saturating_add(30);
                    }
                } else {
                    rgba[idx..idx+4].copy_from_slice(&empty_bar);
                    if y % 3 == 0 {
                        rgba[idx..idx+4].copy_from_slice(&[0x10, 0x10, 0x18, 0xFF]);
                    }
                }
                if y == bar_top || y == bar_bottom - 1 || x == right_x || x == right_x + bar_w - 1 {
                    rgba[idx..idx+4].copy_from_slice(&[0x00, 0xFF, 0xCC, 0x60]);
                }
            }

            // Center gap — subtle vertical line
            if (x == gap_x || x == gap_x + 1) && y > bar_top + 1 && y < bar_bottom - 1 {
                rgba[idx..idx+4].copy_from_slice(&[0x00, 0xFF, 0xCC, 0x20]);
            }
        }
    }

    Icon::from_rgba(rgba, size, size).expect("Failed to create icon")
}

pub fn create_tray(_config: Arc<Mutex<AppConfig>>) -> TrayIcon {
    let quit = MenuItem::new("Quit Claude Tank", true, None);
    let refresh = MenuItem::new("Refresh Now", true, None);
    let about = MenuItem::new("Quatrex / Claude Tank v1.0", false, None);

    let quit_id = quit.id().clone();

    let menu = Menu::with_items(&[
        &refresh,
        &PredefinedMenuItem::separator(),
        &about,
        &quit,
    ]).expect("Failed to create menu");

    let icon = generate_cyber_icon(100.0, 100.0);

    let tray = TrayIconBuilder::new()
        .with_tooltip("Claude Tank\nConnecting...")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .expect("Failed to create tray icon");

    std::thread::spawn(move || {
        loop {
            if let Ok(event) = MenuEvent::receiver().recv() {
                if event.id == quit_id {
                    std::process::exit(0);
                }
            }
        }
    });

    tray
}

/// Update icon and tooltip with real usage data (utilization = consumed %)
pub fn update_tray(tray: &TrayIcon, five_hour_util: f64, seven_day_util: f64) {
    let r5 = 100.0 - five_hour_util;
    let r7 = 100.0 - seven_day_util;

    let icon = generate_cyber_icon(r5, r7);
    let _ = tray.set_icon(Some(icon));
    let _ = tray.set_tooltip(Some(&format!(
        "Claude Tank\n5h: {:.0}% left | 7d: {:.0}% left", r5, r7
    )));
}
