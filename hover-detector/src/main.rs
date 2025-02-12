use anyhow::Result;
use chrono::Local;
use image::{ImageBuffer, Rgb, RgbaImage};
use log::{error, LevelFilter};
use std::{fs::OpenOptions, io::Write, env};
use windows::{
    Win32::Foundation::{POINT, RECT},
    Win32::UI::WindowsAndMessaging::GetCursorPos,
    Win32::UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE},
};
use xcap::Window;
use active_win_pos_rs::get_active_window;

const VERTICAL_THRESHOLD: f64 = 60.0; // Maximum pixels from top of window
const LOG_FILE: &str = "hover_detector.log";
const TARGET_COLORS: [u32; 8] = [0xEE5FB7, 0x4A89BA, 0xCF87DA, 0x69A1FA, 0x84817E, 0x4CB4B7, 0xDF8E64, 0xC1A256];
const BACKGROUND_COLOR: u32 = 0x202020;
const PROXIMITY_RADIUS: i32 = 2; // Radius in pixels to check around cursor for target colors

fn is_verbose() -> bool {
    env::var("TABGROUP_HOVER_DETECTOR_VERBOSE").is_ok()
}

fn log_to_file(msg: &str) -> Result<()> {
    if !is_verbose() {
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE)?;
    writeln!(file, "[{}] {}", Local::now().format("%Y-%m-%d %H:%M:%S"), msg)?;
    Ok(())
}

fn get_cursor_pos() -> Result<POINT> {
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point).ok()?;
    }
    Ok(point)
}

fn get_pixel_color(img: &RgbaImage, x: u32, y: u32) -> Option<u32> {
    if x < img.width() && y < img.height() {
        let pixel = img.get_pixel(x, y);
        let [r, g, b, _] = pixel.0;
        Some(((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    } else {
        None
    }
}

fn save_screenshot(
    img: &RgbaImage,
    scan_y: u32,
    cursor_x: u32,
    cursor_y: u32,
    groups: &[(u32, u32)],
    timestamp: &str,
) -> Result<()> {
    let height = VERTICAL_THRESHOLD as u32;
    let mut debug_img = ImageBuffer::new(img.width(), height);

    // Copy pixels from captured image
    for y in 0..height {
        for x in 0..img.width() {
            if let Some(color) = get_pixel_color(img, x, y) {
                let r = (color >> 16) & 0xFF;
                let g = (color >> 8) & 0xFF;
                let b = color & 0xFF;
                debug_img.put_pixel(x, y, Rgb([r as u8, g as u8, b as u8]));
            }
        }
    }

    // Draw scan line
    if scan_y < height {
        for x in 0..img.width() {
            debug_img.put_pixel(x, scan_y, Rgb([255, 0, 0]));
        }
    }

    // Draw cursor position
    if cursor_y < height {
        for x in cursor_x.saturating_sub(5)..=cursor_x.saturating_add(5) {
            if x < img.width() {
                debug_img.put_pixel(x, cursor_y, Rgb([0, 255, 0]));
            }
        }
        for y in cursor_y.saturating_sub(5)..=cursor_y.saturating_add(5) {
            if y < height {
                debug_img.put_pixel(cursor_x, y, Rgb([0, 255, 0]));
            }
        }
    }

    // Draw group boundaries
    for (start, end) in groups {
        if *start < img.width() {
            for y in 0..height {
                debug_img.put_pixel(*start, y, Rgb([0, 0, 255]));
            }
        }
        if *end < img.width() {
            for y in 0..height {
                debug_img.put_pixel(*end, y, Rgb([0, 0, 255]));
            }
        }
    }

    debug_img.save(format!("screenshot_{}.png", timestamp))?;
    Ok(())
}

fn get_hovered_tab_group_index() -> Result<u32> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    log_to_file(&format!("Starting hover detection at {}", timestamp))?;

    // Get active window first
    let active_window = get_active_window().map_err(|_| anyhow::anyhow!("Failed to get active window"))?;

    // Log active window details
    log_to_file(&format!(
        "Active window details: title='{}', path={:?}, id={}, pos=({}, {}), size={}x{}", 
        active_window.title,
        active_window.process_path,
        active_window.window_id,
        active_window.position.x,
        active_window.position.y,
        active_window.position.width,
        active_window.position.height
    ))?;

    // Get all windows
    let windows = Window::all()?;
    
    // Log all windows for debugging
    for window in &windows {
        log_to_file(&format!(
            "Window state: id={}, title='{}', app_name='{}', focused={}", 
            window.id(), window.title(), window.app_name(), window.is_focused()
        ))?;
    }

    // Determine the window to use for hover detection
    let focused_window = if active_window.title.is_empty() 
        && active_window.process_path
            .file_name()
            .and_then(|f| f.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default()
            .contains("msedge")
    {
        let popup_x = active_window.position.x as i32;
        let popup_y = active_window.position.y as i32;
        
        log_to_file(&format!(
            "Detected Edge popup window at ({}, {}), searching for parent Edge window",
            popup_x, popup_y
        ))?;
        
        // Find Edge window that is slightly above and to the left of the popup
        let y_threshold = 50;
        let x_tolorance = 50;
        let edge_window = windows
            .iter()
            .filter(|w| w.app_name().to_lowercase().contains("edge"))
            .filter(|w| !w.title().is_empty()) // Exclude the popup itself
            .filter(|w| {
                let y_diff = popup_y - w.y(); // Positive if popup is below window
                y_diff > 0 && y_diff < y_threshold // Popup must be below but within threshold
            })
            .filter(|w| w.x() < popup_x + x_tolorance) // Window must be to the left of popup
            .min_by_key(|w| popup_x - w.x()) // Find closest window from the left
            .ok_or_else(|| anyhow::anyhow!("No Edge window found"))?;
        
        log_to_file(&format!(
            "Selected Edge window based on popup: title='{}', pos=({}, {})", 
            edge_window.title(), edge_window.x(), edge_window.y()
        ))?;

        edge_window
    } else {
        // Use normal focused window detection
        windows
            .iter()
            .find(|w| w.is_focused())
            .ok_or_else(|| anyhow::anyhow!("No focused window found"))?
    };
    
    log_to_file(&format!("Selected window for hover detection: '{}' ({})", 
        focused_window.title(), focused_window.app_name()))?;
    
    // Check if it's a browser window by app name
    let app_name = focused_window.app_name().to_lowercase();
    if !app_name.contains("edge") && !app_name.contains("chrome") {
        log_to_file("Not a browser window")?;
        return Ok(0);
    }

    let bounds = RECT {
        left: focused_window.x(),
        top: focused_window.y(),
        right: focused_window.x() + focused_window.width() as i32,
        bottom: focused_window.y() + VERTICAL_THRESHOLD as i32,
    };
    
    log_to_file(&format!("Window bounds: left={}, top={}, right={}, bottom={}", 
        bounds.left, bounds.top, bounds.right, bounds.bottom))?;
    
    // Get cursor position
    let cursor = get_cursor_pos()?;
    log_to_file(&format!("Cursor position: x={}, y={}", cursor.x, cursor.y))?;
    
    // Check if cursor is within tab group area
    if cursor.x < bounds.left
        || cursor.x > bounds.right
        || cursor.y < bounds.top
        || cursor.y > bounds.bottom
    {
        log_to_file("Cursor outside tab group area")?;
        return Ok(0);
    }
    
    // Y position to scan for tab groups (halfway up the title bar)
    let scan_y = (VERTICAL_THRESHOLD / 2.0) as u32;
    log_to_file(&format!("Scan line y-position: {}", scan_y))?;
    
    // Take screenshot of the window
    let capture = focused_window.capture_image()?;
    
    // Convert cursor position to image coordinates
    let cursor_x = (cursor.x - bounds.left) as u32;
    let cursor_y = (cursor.y - bounds.top) as u32;
    
    // Save initial screenshot before color detection if in verbose mode
    if is_verbose() {
        save_screenshot(&capture, scan_y, cursor_x, cursor_y, &Vec::new(), &timestamp)?;
    }
    
    // Check if cursor is hovering over a target color (check at scan_y height)
    let mut found_target_color = false;
    'proximity_check: for dx in -PROXIMITY_RADIUS..=PROXIMITY_RADIUS {
        let check_x = cursor_x as i32 + dx;
        if check_x >= 0 && check_x < capture.width() as i32 {
            if let Some(color) = get_pixel_color(&capture, check_x as u32, scan_y) {
                if TARGET_COLORS.contains(&color) {
                    found_target_color = true;
                    log_to_file(&format!("Found target color #{:06x} at x-offset {}", color, dx))?;
                    break 'proximity_check;
                }
            }
        }
    }
    
    if !found_target_color {
        log_to_file("Not hovering on a tab group")?;
        return Ok(0);
    }

    log_to_file(&format!("Checking tab groups at cursor x={}", cursor_x))?;
    
    // Variables to track tab groups
    let mut current_group_index = 0;
    let mut last_color = BACKGROUND_COLOR;
    let mut group_start = 0;
    let mut groups = Vec::new();
    let mut in_group = false;

    // Scan horizontally for tab groups
    for x in 0..capture.width() {
        if let Some(current_color) = get_pixel_color(&capture, x, scan_y) {
            // Only update last_color if current is background or target
            if current_color == BACKGROUND_COLOR || TARGET_COLORS.contains(&current_color) {
                // Detect transitions
                if !in_group && last_color == BACKGROUND_COLOR && TARGET_COLORS.contains(&current_color) {
                    // Start of new tab group
                    current_group_index += 1;
                    in_group = true;
                    group_start = x;
                    log_to_file(&format!("Found tab group {} starting at x={} (color=#{:06x})", 
                        current_group_index, x, current_color))?;
                    
                    // Check if cursor is before this group
                    if cursor_x <= x {
                        log_to_file("Cursor before this group")?;
                        if is_verbose() && !groups.is_empty() {
                            save_screenshot(&capture, scan_y, cursor_x, cursor_y, &groups, &timestamp)?;
                        }
                        return Ok(0);
                    }
                } else if in_group && TARGET_COLORS.contains(&last_color) && current_color == BACKGROUND_COLOR {
                    // End of tab group
                    in_group = false;
                    groups.push((group_start, x));
                    log_to_file(&format!("Tab group {} ends at x={}", current_group_index, x))?;
                    
                    // Check if cursor was in this group
                    if cursor_x <= x {
                        log_to_file(&format!("Cursor in group {}", current_group_index))?;
                        if is_verbose() {
                            save_screenshot(&capture, scan_y, cursor_x, cursor_y, &groups, &timestamp)?;
                        }
                        return Ok(current_group_index);
                    }
                }
                last_color = current_color;
            }
        }
    }
    
    // Handle case where cursor is in last group that extends to window edge
    if in_group {
        groups.push((group_start, capture.width()));
        log_to_file(&format!("Cursor in last group {} (extends to window edge)", current_group_index))?;
        if is_verbose() {
            save_screenshot(&capture, scan_y, cursor_x, cursor_y, &groups, &timestamp)?;
        }
        return Ok(current_group_index);
    }
    
    log_to_file("No tab group found at cursor position")?;
    Ok(0)
}

fn main() -> Result<()> {
    // Initialize logger with custom filter
    env_logger::Builder::new()
        .filter_level(LevelFilter::Off) // Suppress all logs by default
        .filter_module("hover_detector", LevelFilter::Error) // Only show our errors
        .init();
    
    // Make process DPI aware
    unsafe {
        SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE)
            .map_err(|e| anyhow::anyhow!("Failed to set DPI awareness: {}", e))?;
    }
    
    match get_hovered_tab_group_index() {
        Ok(index) => {
            print!("{}", index); // Print just the number for easy parsing
            Ok(())
        }
        Err(e) => {
            error!("Error: {}", e);
            Err(e)
        }
    }
}
