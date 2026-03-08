use anyhow::Result;
use chrono::Local;
use image::{ImageBuffer, Rgb, RgbaImage};
use log::{error, LevelFilter};
use std::{collections::HashMap, env, fs::OpenOptions, io::Write};
use windows::{
    Win32::Foundation::{HWND, POINT, RECT},
    Win32::UI::WindowsAndMessaging::{
        GA_ROOT,
        GA_ROOTOWNER,
        GW_OWNER,
        GetAncestor,
        GetCursorPos,
        GetForegroundWindow,
        GetWindow,
        WindowFromPoint,
    },
    Win32::UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE},
};
use xcap::Window;

const VERTICAL_THRESHOLD: f64 = 60.0; // Maximum pixels from top of window
const LOG_FILE: &str = "hover_detector.log";
const TARGET_COLORS: [u32; 9] = [0x779FF8, 0xE06AB7, 0xC78BD9, 0xB497FE, 0x5987B9, 0x65B1B6, 0xD59367, 0xBCA359, 0x83817E];
const TARGET_COLORS_ALT: [u32; 9] = [0x7BA0FD, 0xDB6ABA, 0xC48BDD, 0xB298FF, 0x5E87BC, 0x6DB1B7, 0xD19262, 0xBAA351, 0x83817E];
const BACKGROUND_COLOR: u32 = 0x202020;
const PROXIMITY_RADIUS: i32 = 2; // Radius in pixels to check around cursor for target colors
const TARGET_COLOR_TOLERANCE: u32 = 20;
const BACKGROUND_COLOR_TOLERANCE: u32 = 18;
const MAX_BACKGROUND_COLORS: usize = 6;
const MIN_GROUP_WIDTH_DEFAULT: u32 = 24;
const MIN_BACKGROUND_GAP_WIDTH_DEFAULT: u32 = 8;

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

fn color_distance(a: u32, b: u32) -> u32 {
    let ar = ((a >> 16) & 0xFF) as i32;
    let ag = ((a >> 8) & 0xFF) as i32;
    let ab = (a & 0xFF) as i32;
    let br = ((b >> 16) & 0xFF) as i32;
    let bg = ((b >> 8) & 0xFF) as i32;
    let bb = (b & 0xFF) as i32;
    ((ar - br).abs() + (ag - bg).abs() + (ab - bb).abs()) as u32
}

fn parse_hex_color(input: &str) -> Option<u32> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .or_else(|| trimmed.strip_prefix('#'))
        .unwrap_or(trimmed);
    if normalized.len() != 6 {
        return None;
    }
    u32::from_str_radix(normalized, 16).ok()
}

fn parse_colors_from_env(var_name: &str) -> Vec<u32> {
    let Ok(raw) = env::var(var_name) else {
        return Vec::new();
    };
    raw.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter_map(parse_hex_color)
        .collect()
}

fn parse_u32_from_env(var_name: &str, default_value: u32) -> u32 {
    env::var(var_name)
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
        .unwrap_or(default_value)
}

fn target_colors() -> Vec<u32> {
    let mut colors = Vec::with_capacity(TARGET_COLORS.len() + TARGET_COLORS_ALT.len() + 8);
    colors.extend(TARGET_COLORS);
    colors.extend(TARGET_COLORS_ALT);
    colors.extend(parse_colors_from_env("TABGROUP_HOVER_EXTRA_COLORS"));
    colors
}

fn color_channel_spread(color: u32) -> u32 {
    let r = (color >> 16) & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = color & 0xFF;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    max - min
}

fn color_max_channel(color: u32) -> u32 {
    let r = (color >> 16) & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = color & 0xFF;
    r.max(g).max(b)
}

fn is_target_color(color: u32, targets: &[u32]) -> bool {
    targets
        .iter()
        .any(|target| color_distance(color, *target) <= TARGET_COLOR_TOLERANCE)
}

fn is_background_color(color: u32, background_candidates: &[u32]) -> bool {
    background_candidates
        .iter()
        .any(|candidate| color_distance(color, *candidate) <= BACKGROUND_COLOR_TOLERANCE)
}

fn resolve_background_candidates(img: &RgbaImage, scan_y: u32, targets: &[u32]) -> Vec<u32> {
    let user_candidates = parse_colors_from_env("TABGROUP_HOVER_BG_COLORS");
    if !user_candidates.is_empty() {
        return user_candidates;
    }

    let mut counts: HashMap<u32, u32> = HashMap::new();
    for x in 0..img.width() {
        if let Some(color) = get_pixel_color(img, x, scan_y) {
            if !is_target_color(color, targets) {
                *counts.entry(color).or_insert(0) += 1;
            }
        }
    }

    let min_count = ((img.width() as f64) * 0.005).max(6.0) as u32;
    let mut sorted: Vec<(u32, u32)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let mut candidates = Vec::new();
    for (color, count) in sorted {
        if count < min_count {
            break;
        }
        let spread = color_channel_spread(color);
        let max_channel = color_max_channel(color);
        if spread <= 28 && max_channel <= 120 {
            candidates.push(color);
            if candidates.len() >= MAX_BACKGROUND_COLORS {
                break;
            }
        }
    }

    if candidates.is_empty() {
        candidates.push(BACKGROUND_COLOR);
    }
    candidates
}

fn is_browser_app_name(app_name: &str) -> bool {
    app_name.contains("edge") || app_name.contains("chrome")
}

fn is_point_in_window(cursor: POINT, window: &Window) -> bool {
    let left = window.x();
    let top = window.y();
    let right = left + window.width() as i32;
    let bottom = top + window.height() as i32;

    cursor.x >= left && cursor.x < right && cursor.y >= top && cursor.y < bottom
}

fn push_unique_handle(handles: &mut Vec<HWND>, hwnd: HWND) {
    if hwnd.0 != 0 && !handles.iter().any(|h| h.0 == hwnd.0) {
        handles.push(hwnd);
    }
}

fn add_handle_candidates(handles: &mut Vec<HWND>, start: HWND) {
    if start.0 == 0 {
        return;
    }

    push_unique_handle(handles, start);

    unsafe {
        push_unique_handle(handles, GetAncestor(start, GA_ROOTOWNER));
        push_unique_handle(handles, GetAncestor(start, GA_ROOT));
    }

    let mut current = start;
    for _ in 0..8 {
        unsafe {
            let owner = GetWindow(current, GW_OWNER);
            if owner.0 == 0 || owner.0 == current.0 {
                break;
            }
            push_unique_handle(handles, owner);
            push_unique_handle(handles, GetAncestor(owner, GA_ROOTOWNER));
            push_unique_handle(handles, GetAncestor(owner, GA_ROOT));
            current = owner;
        }
    }
}

fn resolve_browser_window<'a>(windows: &'a [Window], cursor: POINT) -> Result<&'a Window> {
    let mut candidates = Vec::new();
    unsafe {
        add_handle_candidates(&mut candidates, WindowFromPoint(cursor));
        add_handle_candidates(&mut candidates, GetForegroundWindow());
    }

    if is_verbose() {
        let handles = candidates
            .iter()
            .map(|h| format!("{}", h.0))
            .collect::<Vec<_>>()
            .join(", ");
        log_to_file(&format!("HWND candidates: [{}]", handles))?;
    }

    for hwnd in &candidates {
        if let Some(window) = windows.iter().find(|w| w.id() as isize == hwnd.0) {
            let app_name = window.app_name().to_lowercase();
            if is_browser_app_name(&app_name) && !window.title().is_empty() {
                log_to_file(&format!(
                    "Resolved browser via HWND chain: id={}, title='{}', app='{}'",
                    window.id(),
                    window.title(),
                    window.app_name()
                ))?;
                return Ok(window);
            }
        }
    }

    if let Some(window) = windows
        .iter()
        .filter(|w| is_browser_app_name(&w.app_name().to_lowercase()))
        .filter(|w| !w.title().is_empty())
        .filter(|w| is_point_in_window(cursor, w))
        .max_by_key(|w| (w.width() as u64) * (w.height() as u64))
    {
        log_to_file(&format!(
            "Resolved browser via cursor containment fallback: id={}, title='{}', app='{}'",
            window.id(),
            window.title(),
            window.app_name()
        ))?;
        return Ok(window);
    }

    if let Some(window) = windows
        .iter()
        .find(|w| w.is_focused() && is_browser_app_name(&w.app_name().to_lowercase()))
    {
        log_to_file(&format!(
            "Resolved browser via focused window fallback: id={}, title='{}', app='{}'",
            window.id(),
            window.title(),
            window.app_name()
        ))?;
        return Ok(window);
    }

    Err(anyhow::anyhow!(
        "No Edge/Chrome window found for hover detection"
    ))
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
    let cursor = get_cursor_pos()?;
    log_to_file(&format!("Cursor position: x={}, y={}", cursor.x, cursor.y))?;

    // Get all windows
    let windows = Window::all()?;
    
    // Log all windows for debugging
    for window in &windows {
        log_to_file(&format!(
            "Window state: id={}, title='{}', app_name='{}', focused={}", 
            window.id(), window.title(), window.app_name(), window.is_focused()
        ))?;
    }

    let focused_window = match resolve_browser_window(&windows, cursor) {
        Ok(window) => window,
        Err(e) => {
            log_to_file(&format!("Browser window resolution failed: {}", e))?;
            return Ok(0);
        }
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
    
    // Check if cursor is within tab group area
    if cursor.x < bounds.left
        || cursor.x >= bounds.right
        || cursor.y < bounds.top
        || cursor.y >= bounds.bottom
    {
        log_to_file("Cursor outside tab group area")?;
        return Ok(0);
    }
    
    // Y position to scan for tab groups (halfway up the title bar)
    let scan_y = (VERTICAL_THRESHOLD / 2.0) as u32;
    log_to_file(&format!("Scan line y-position: {}", scan_y))?;
    
    // Take screenshot of the window
    let capture = focused_window.capture_image()?;
    let targets = target_colors();
    let background_candidates = resolve_background_candidates(&capture, scan_y, &targets);
    let min_group_width = parse_u32_from_env(
        "TABGROUP_HOVER_MIN_GROUP_WIDTH",
        MIN_GROUP_WIDTH_DEFAULT,
    );
    let min_bg_gap_width = parse_u32_from_env(
        "TABGROUP_HOVER_MIN_BG_GAP_WIDTH",
        MIN_BACKGROUND_GAP_WIDTH_DEFAULT,
    );
    log_to_file(&format!(
        "Using {} target colors and {} background candidates: [{}], min_group_width={}, min_bg_gap_width={}",
        targets.len(),
        background_candidates.len(),
        background_candidates
            .iter()
            .map(|c| format!("#{:06X}", c))
            .collect::<Vec<_>>()
            .join(", "),
        min_group_width,
        min_bg_gap_width
    ))?;
    
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
                if is_target_color(color, &targets) {
                    found_target_color = true;
                    log_to_file(&format!("Found target color #{:06x} at x-offset {}", color, dx))?;
                    break 'proximity_check;
                } else {
                    log_to_file(&format!("Color at x-offset {} is not a target color: #{:06x}", dx, color))?;
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
    let mut groups = Vec::new();
    let mut active_group_start: Option<u32> = None;
    let mut pending_bg_start: Option<u32> = None;

    // Scan horizontally for tab groups
    for x in 0..capture.width() {
        if let Some(current_color) = get_pixel_color(&capture, x, scan_y) {
            let current_is_target = is_target_color(current_color, &targets);
            let current_is_background = is_background_color(current_color, &background_candidates);

            if active_group_start.is_none() {
                if current_is_target {
                    active_group_start = Some(x);
                    pending_bg_start = None;
                }
                continue;
            }

            if current_is_target {
                pending_bg_start = None;
                continue;
            }

            if current_is_background {
                if pending_bg_start.is_none() {
                    pending_bg_start = Some(x);
                }
                let bg_start = pending_bg_start.unwrap_or(x);
                let bg_width = x.saturating_sub(bg_start) + 1;
                if bg_width >= min_bg_gap_width {
                    let group_start = active_group_start.unwrap_or(0);
                    let group_end = bg_start;
                    let group_width = group_end.saturating_sub(group_start);
                    if group_width >= min_group_width {
                        groups.push((group_start, group_end));
                        log_to_file(&format!(
                            "Accepted tab group {}: start={}, end={}, width={}",
                            groups.len(),
                            group_start,
                            group_end,
                            group_width
                        ))?;
                    } else {
                        log_to_file(&format!(
                            "Ignored narrow group candidate: start={}, end={}, width={}",
                            group_start,
                            group_end,
                            group_width
                        ))?;
                    }
                    active_group_start = None;
                    pending_bg_start = None;
                }
            }
        }
    }
    
    // Handle case where cursor is in last group that extends to window edge
    if let Some(group_start) = active_group_start {
        let group_end = capture.width();
        let group_width = group_end.saturating_sub(group_start);
        if group_width >= min_group_width {
            groups.push((group_start, group_end));
            log_to_file(&format!(
                "Accepted trailing tab group {}: start={}, end={}, width={}",
                groups.len(),
                group_start,
                group_end,
                group_width
            ))?;
        } else {
            log_to_file(&format!(
                "Ignored narrow trailing group candidate: start={}, end={}, width={}",
                group_start,
                group_end,
                group_width
            ))?;
        }
    }

    if is_verbose() {
        save_screenshot(&capture, scan_y, cursor_x, cursor_y, &groups, &timestamp)?;
    }

    for (index, (start, end)) in groups.iter().enumerate() {
        if cursor_x >= *start && cursor_x < *end {
            let group_index = (index + 1) as u32;
            log_to_file(&format!(
                "Cursor in accepted group {} (range {}..{})",
                group_index, start, end
            ))?;
            return Ok(group_index);
        }
    }

    log_to_file("No accepted tab group found at cursor position")?;
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
