use crate::config::AppConfig;
use std::sync::{Arc, Mutex};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

// Icon dimensions
const ICON_SIZE: u32 = 32;
const BAR_WIDTH: u32 = 11;
const BAR_TOP: u32 = 3;
const BAR_BOTTOM: u32 = 29;
const LEFT_BAR_X: u32 = 2;
const RIGHT_BAR_X: u32 = 19;
const GAP_X: u32 = 15;

// Cyber theme colors
const BG: [u8; 4] = [0x0A, 0x0A, 0x0F, 0xFF];
const BORDER: [u8; 4] = [0x00, 0xFF, 0xCC, 0x40];
const EMPTY_BAR: [u8; 4] = [0x15, 0x15, 0x20, 0xFF];
const SCANLINE: [u8; 4] = [0x10, 0x10, 0x18, 0xFF];
const BAR_BORDER: [u8; 4] = [0x00, 0xFF, 0xCC, 0x60];
const GAP_COLOR: [u8; 4] = [0x00, 0xFF, 0xCC, 0x20];

/// Remaining% → neon color
fn cyber_color(remaining_pct: f64) -> [u8; 4] {
    if remaining_pct > 50.0 { [0x00, 0xFF, 0xCC, 0xFF] }       // cyan
    else if remaining_pct > 20.0 { [0xFF, 0xD7, 0x00, 0xFF] }  // gold
    else if remaining_pct > 5.0 { [0xFF, 0x00, 0xFF, 0xFF] }   // magenta
    else { [0xFF, 0x22, 0x44, 0xFF] }                           // neon red
}

/// Draw one vertical bar into the RGBA buffer
fn draw_bar(rgba: &mut [u8], bar_x: u32, fill_pct: f64, color: [u8; 4]) {
    let bar_h = BAR_BOTTOM - BAR_TOP;
    let fill_h = (bar_h as f64 * fill_pct / 100.0).round() as u32;
    let fill_line = BAR_BOTTOM.saturating_sub(fill_h);

    for y in BAR_TOP..BAR_BOTTOM {
        for x in bar_x..bar_x + BAR_WIDTH {
            let idx = ((y * ICON_SIZE + x) * 4) as usize;

            // Border
            if y == BAR_TOP || y == BAR_BOTTOM - 1 || x == bar_x || x == bar_x + BAR_WIDTH - 1 {
                rgba[idx..idx + 4].copy_from_slice(&BAR_BORDER);
                continue;
            }

            if y >= fill_line {
                // Filled area
                rgba[idx..idx + 4].copy_from_slice(&color);
                // Glow: brighter center
                if x >= bar_x + 3 && x < bar_x + BAR_WIDTH - 3 {
                    rgba[idx] = rgba[idx].saturating_add(30);
                    rgba[idx + 1] = rgba[idx + 1].saturating_add(30);
                    rgba[idx + 2] = rgba[idx + 2].saturating_add(30);
                }
            } else {
                // Empty area with scanline
                let c = if y % 3 == 0 { SCANLINE } else { EMPTY_BAR };
                rgba[idx..idx + 4].copy_from_slice(&c);
            }
        }
    }
}

/// Generate 32x32 cyber-themed dual gauge icon
fn generate_icon(five_hour_remaining: f64, seven_day_remaining: f64) -> Icon {
    let mut rgba = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    // Background + outer border
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

    // Bars
    draw_bar(&mut rgba, LEFT_BAR_X, five_hour_remaining, cyber_color(five_hour_remaining));
    draw_bar(&mut rgba, RIGHT_BAR_X, seven_day_remaining, cyber_color(seven_day_remaining));

    // Center gap line
    for y in BAR_TOP + 2..BAR_BOTTOM - 1 {
        for x in GAP_X..=GAP_X + 1 {
            let idx = ((y * ICON_SIZE + x) * 4) as usize;
            rgba[idx..idx + 4].copy_from_slice(&GAP_COLOR);
        }
    }

    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE).expect("Failed to create icon")
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

    let tray = TrayIconBuilder::new()
        .with_tooltip("Claude Tank\nConnecting...")
        .with_icon(generate_icon(100.0, 100.0))
        .with_menu(Box::new(menu))
        .build()
        .expect("Failed to create tray icon");

    std::thread::spawn(move || {
        while let Ok(event) = MenuEvent::receiver().recv() {
            if event.id == quit_id {
                std::process::exit(0);
            }
        }
    });

    tray
}

/// Update icon and tooltip (utilization = consumed %, we show remaining)
pub fn update_tray(tray: &TrayIcon, five_hour_util: f64, seven_day_util: f64) {
    let r5 = 100.0 - five_hour_util;
    let r7 = 100.0 - seven_day_util;
    let _ = tray.set_icon(Some(generate_icon(r5, r7)));
    let _ = tray.set_tooltip(Some(&format!(
        "Claude Tank\n5h: {:.0}% left | 7d: {:.0}% left", r5, r7
    )));
}
