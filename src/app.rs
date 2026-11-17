use std::cell::RefCell;
use std::mem::{size_of, zeroed};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::time::{SystemTime, UNIX_EPOCH};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::System::Time::*;
use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::{hook, icon, refresh as auto, snapshot, sysstats, view};

const WM_TRAY: u32 = WM_APP + 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const TRAY_ID: u32 = 1;
const CMD_TOGGLE: usize = 1;
const CMD_EXIT: usize = 2;
const CMD_LABELS: usize = 3;
const CMD_AUTO: usize = 4;
const CMD_STARTUP: usize = 5;
const CMD_CPU: usize = 6;
const CMD_RAM: usize = 7;
const CMD_GPU: usize = 8;
const CMD_NET: usize = 9;
const STATS_TIMER: usize = 1;
const STATS_INTERVAL_MS: u32 = 2000;
const MIN_W: i32 = 110;
const CMD_RESET_SIZE: usize = 10;
const WM_REFRESH_DONE: u32 = WM_APP + 2;
const STATS_FILE: &str = "refresh-stats.txt";
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "TokenStat";
const IMPACT: &str = "When you use Claude in the VS Code extension, TokenStat cannot see the new usage. With auto-refresh on, TokenStat sends one tiny hidden Claude Code request (Haiku, reply \"ok\") to fetch fresh figures, at most once every 5 minutes, and only after you have used Claude somewhere TokenStat did not see.\n\nImpact:\n- Each refresh uses a small amount of your Claude allowance, like a short message.\n- Each refresh is a few seconds of background work.\n- Refreshes appear in your account usage, including any reports your admin can see.\n- It relies on an undocumented Claude Code field. If that changes, readings show as unavailable, never wrong.\n\nThe menu shows how many refreshes ran today and the tokens they used.\n\nTurn on auto-refresh?";
const BASE_W: i32 = 250;
const SETTINGS_FILE: &str = "settings.txt";
const BASE_H: i32 = 30;
const SNAPSHOT_FILES: [&str; 1] = [snapshot::FILE_NAME];
const POS_FILE: &str = "pill.pos";
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

struct App {
    hwnd: HWND,
    dir: PathBuf,
    snap: Option<snapshot::Snapshot>,
    load_error: bool,
    view: view::View,
    font: HFONT,
    dpi: u32,
    timer: HANDLE,
    taskbar_created: u32,
    labels: bool,
    tray_icon: HICON,
    auto_refresh: bool,
    refreshing: bool,
    next_refresh_ok: i64,
    failures: u32,
    activity_at: Option<i64>,
    sys_on: sysstats::Enabled,
    sys: sysstats::Stats,
    sampler: sysstats::Sampler,
    /// User-chosen pill width in 96-dpi units; None means fit everything on one row.
    width: Option<i32>,
    drag: Option<(bool, RECT, POINT)>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

// Never call a Win32 function that can send messages to our window inside `f`.
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn scale(v: i32, dpi: u32) -> i32 {
    v * dpi as i32 / 96
}

fn loword(v: usize) -> u32 {
    (v & 0xFFFF) as u32
}

fn signed_lo(v: usize) -> i32 {
    (v & 0xFFFF) as u16 as i16 as i32
}

fn signed_hi(v: usize) -> i32 {
    ((v >> 16) & 0xFFFF) as u16 as i16 as i32
}

fn local_parts(t: i64) -> SYSTEMTIME {
    unsafe {
        let ticks = ((t + 11_644_473_600) * 10_000_000) as u64;
        let ft = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
        let mut utc: SYSTEMTIME = zeroed();
        let mut local: SYSTEMTIME = zeroed();
        FileTimeToSystemTime(&ft, &mut utc);
        SystemTimeToTzSpecificLocalTime(null(), &utc, &mut local);
        local
    }
}

fn format_local(t: i64, now: i64) -> String {
    let l = local_parts(t);
    let n = local_parts(now);
    let h = match l.wHour % 12 {
        0 => 12,
        h => h,
    };
    let time = format!("{}:{:02} {}", h, l.wMinute, if l.wHour < 12 { "am" } else { "pm" });
    if (l.wYear, l.wMonth, l.wDay) == (n.wYear, n.wMonth, n.wDay) {
        time
    } else {
        format!("{} {}", DAYS[(l.wDayOfWeek % 7) as usize], time)
    }
}

fn read_snapshot(dir: &Path) -> Result<Option<snapshot::Snapshot>, String> {
    for name in SNAPSHOT_FILES {
        match std::fs::read(dir.join(name)) {
            Ok(bytes) => return snapshot::parse(&bytes).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(None)
}

fn make_font(dpi: u32) -> HFONT {
    let face = wide("Segoe UI");
    unsafe {
        CreateFontW(
            -scale(13, dpi),
            0,
            0,
            0,
            FW_SEMIBOLD as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            0,
            face.as_ptr(),
        )
    }
}

fn high_contrast() -> bool {
    unsafe {
        let mut hc: HIGHCONTRASTW = zeroed();
        hc.cbSize = size_of::<HIGHCONTRASTW>() as u32;
        SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, &mut hc as *mut _ as _, 0) != 0
            && hc.dwFlags & HCF_HIGHCONTRASTON != 0
    }
}

fn tray_data(hwnd: HWND, tip: &str, icon: HICON) -> NOTIFYICONDATAW {
    unsafe {
        let mut nid: NOTIFYICONDATAW = zeroed();
        nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ID;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = icon;
        for (i, c) in tip.encode_utf16().take(nid.szTip.len() - 1).enumerate() {
            nid.szTip[i] = c;
        }
        nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        nid
    }
}

fn add_tray(hwnd: HWND, tip: &str, icon: HICON) {
    let nid = tray_data(hwnd, tip, icon);
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &nid);
        Shell_NotifyIconW(NIM_SETVERSION, &nid);
    }
}

fn set_timer(timer: HANDLE, next_change: Option<i64>, now: i64) {
    unsafe {
        match next_change {
            Some(t) if t > now => {
                let due: i64 = -((t - now + 1) * 10_000_000);
                SetWaitableTimer(timer, &due, 0, None, null(), 0);
            }
            _ => {
                CancelWaitableTimer(timer);
            }
        }
    }
}

/// Rebuild the view, optionally re-reading the snapshot file, then update the screen.
fn refresh(reload: bool) {
    let t = now();
    let Some((hwnd, tip, accessible, rings)) = with_app(|a| {
        if reload {
            match read_snapshot(&a.dir) {
                Ok(s) => {
                    a.snap = s;
                    a.load_error = false;
                }
                Err(_) => a.load_error = true,
            }
            a.activity_at = read_activity(&a.dir);
        }
        let unseen = auto::unseen_activity(a.activity_at, a.snap.as_ref());
        a.view = view::build_with_activity(a.snap.as_ref(), a.load_error, t, a.labels, unseen, &|x| format_local(x, t));
        let mut wake = a.view.next_change;
        if a.auto_refresh && unseen && !a.refreshing {
            if t >= a.next_refresh_ok {
                start_refresh(a, t);
            } else {
                wake = Some(wake.map_or(a.next_refresh_ok, |w| w.min(a.next_refresh_ok)));
            }
        }
        set_timer(a.timer, wake, t);
        (a.hwnd, a.view.tooltip.clone(), a.view.accessible.clone(), (a.view.rings, a.view.freshness))
    }) else {
        return;
    };
    let new_icon = make_tray_icon(rings.0, rings.1);
    let old_icon = with_app(|a| std::mem::replace(&mut a.tray_icon, new_icon)).unwrap_or(null_mut());
    unsafe {
        let nid = tray_data(hwnd, &tip, new_icon);
        Shell_NotifyIconW(NIM_MODIFY, &nid);
        if !old_icon.is_null() {
            DestroyIcon(old_icon);
        }
        SetWindowTextW(hwnd, wide(&accessible).as_ptr());
        InvalidateRect(hwnd, null(), 1);
    }
    resize_to_content(hwnd);
}

fn read_activity(dir: &Path) -> Option<i64> {
    std::fs::read_to_string(dir.join(hook::ACTIVITY_FILE)).ok()?.trim().parse().ok()
}

fn load_number(key: &str) -> Option<i32> {
    let text = std::fs::read_to_string(snapshot::data_dir().join(SETTINGS_FILE)).ok()?;
    text.lines().find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))?.trim().parse().ok()
}

fn load_setting(key: &str, default: bool) -> bool {
    let text = std::fs::read_to_string(snapshot::data_dir().join(SETTINGS_FILE)).unwrap_or_default();
    text.lines()
        .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
        .map(|v| v.trim() == "1")
        .unwrap_or(default)
}

fn save_settings() {
    if let Some((labels, refresh, on, width)) = with_app(|a| (a.labels, a.auto_refresh, a.sys_on, a.width)) {
        let text = format!(
            "labels={}\nrefresh={}\ncpu={}\nram={}\ngpu={}\nnet={}\nwidth={}\n",
            labels as u8,
            refresh as u8,
            on.cpu as u8,
            on.ram as u8,
            on.gpu as u8,
            on.net as u8,
            width.unwrap_or(0)
        );
        let _ = std::fs::write(snapshot::data_dir().join(SETTINGS_FILE), text);
    }
}

fn today() -> String {
    let l = local_parts(now());
    format!("{:04}-{:02}-{:02}", l.wYear, l.wMonth, l.wDay)
}

/// (refreshes today, tokens used today)
fn read_stats() -> (u64, u64) {
    let text = std::fs::read_to_string(snapshot::data_dir().join(STATS_FILE)).unwrap_or_default();
    let mut it = text.split_whitespace();
    if it.next() != Some(today().as_str()) {
        return (0, 0);
    }
    let mut n = || it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    (n(), n())
}

fn add_stats(tokens: u64) {
    let (count, total) = read_stats();
    let _ = std::fs::write(snapshot::data_dir().join(STATS_FILE), format!("{} {} {}", today(), count + 1, total + tokens));
}

fn start_refresh(a: &mut App, t: i64) {
    a.refreshing = true;
    let hwnd = a.hwnd as isize;
    let dir = a.dir.clone();
    std::thread::spawn(move || {
        let ok = match auto::run(&dir.join("refresh"), t) {
            Ok(o) => {
                add_stats(o.tokens);
                hook::write_atomic(&dir, &o.snapshot).is_ok()
            }
            Err(_) => false,
        };
        unsafe {
            PostMessageW(hwnd as HWND, WM_REFRESH_DONE, ok as usize, 0);
        }
    });
}

fn refresh_done(ok: bool) {
    with_app(|a| {
        a.refreshing = false;
        a.failures = if ok { 0 } else { a.failures + 1 };
        a.next_refresh_ok = now() + auto::backoff(a.failures);
    });
    refresh(true);
}

fn toggle_auto_refresh(hwnd: HWND) {
    let on = with_app(|a| a.auto_refresh).unwrap_or(false);
    if !on {
        let text = wide(IMPACT);
        let title = wide("TokenStat: auto-refresh after VS Code use");
        let answer = unsafe { MessageBoxW(hwnd, text.as_ptr(), title.as_ptr(), MB_OKCANCEL | MB_ICONINFORMATION) };
        if answer != IDOK {
            return;
        }
    }
    with_app(|a| a.auto_refresh = !on);
    save_settings();
    refresh(false);
}

fn exe_path() -> String {
    let mut buf = [0u16; 1024];
    let n = unsafe { GetModuleFileNameW(null_mut(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    String::from_utf16_lossy(&buf[..n])
}

fn startup_enabled() -> bool {
    let key = wide(RUN_KEY);
    let name = wide(RUN_VALUE);
    let mut size = 0u32;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, null_mut(), null_mut(), &mut size) == 0 }
}

fn toggle_startup() {
    let key = wide(RUN_KEY);
    let name = wide(RUN_VALUE);
    unsafe {
        if startup_enabled() {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr());
        } else {
            let value = wide(&format!("\"{}\"", exe_path()));
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                value.as_ptr() as _,
                (value.len() * 2) as u32,
            );
        }
    }
}

fn line_width(hdc: HDC, segments: &[view::Segment]) -> i32 {
    segments
        .iter()
        .map(|(t, _)| {
            let w: Vec<u16> = t.encode_utf16().collect();
            let mut sz: SIZE = unsafe { zeroed() };
            unsafe { GetTextExtentPoint32W(hdc, w.as_ptr(), w.len() as i32, &mut sz) };
            sz.cx
        })
        .sum()
}

const ROW_H: i32 = 22;
const PAD_Y: i32 = 4;
const GAP: i32 = 16;
const PAD_L: i32 = 28;
const PAD_R: i32 = 12;

/// Everything on the pill as wrap-able items: Claude figures first, then each enabled system stat.
fn pill_items(a: &App) -> Vec<Vec<view::Segment>> {
    let mut items = a.view.items.clone();
    if a.sys_on.any() {
        items.extend(view::system_items(&a.sys, a.sys_on, a.labels));
    }
    items
}

/// Pill size for the user's chosen width (96-dpi units), or on one row when no width is set.
fn layout(hdc: HDC, items: &[Vec<view::Segment>], dpi: u32, width: Option<i32>) -> (i32, i32) {
    let widths: Vec<i32> = items.iter().map(|i| line_width(hdc, i)).collect();
    let gap = scale(GAP, dpi);
    let pads = scale(PAD_L + PAD_R, dpi);
    let widest = widths.iter().copied().max().unwrap_or(0);
    let natural = widths.iter().sum::<i32>() + gap * (widths.len() as i32 - 1).max(0);
    let total = match width {
        Some(w) => scale(w, dpi).max(widest + pads).max(scale(MIN_W, dpi)),
        None => (natural + pads).max(scale(MIN_W, dpi)),
    };
    let rows = view::wrap(&widths, total - pads, gap);
    let height = rows.len().max(1) as i32 * scale(ROW_H, dpi) + 2 * scale(PAD_Y, dpi);
    (total, height)
}

/// Pill size in physical pixels.
fn pill_size(hwnd: HWND, dpi: u32) -> (i32, i32) {
    let Some((items, font, width)) = with_app(|a| (pill_items(a), a.font, a.width)) else {
        return (scale(BASE_W, dpi), scale(BASE_H, dpi));
    };
    unsafe {
        let hdc = GetDC(hwnd);
        let old = SelectObject(hdc, font as _);
        let size = layout(hdc, &items, dpi, width);
        SelectObject(hdc, old);
        ReleaseDC(hwnd, hdc);
        size
    }
}

fn resize_edge_hit(hwnd: HWND, x: i32) -> bool {
    unsafe {
        let mut r: RECT = zeroed();
        GetWindowRect(hwnd, &mut r);
        let edge = scale(6, GetDpiForWindow(hwnd).max(96));
        x < r.left + edge || x >= r.right - edge
    }
}

fn begin_resize(hwnd: HWND) {
    unsafe {
        let mut pt: POINT = zeroed();
        let mut r: RECT = zeroed();
        GetCursorPos(&mut pt);
        GetWindowRect(hwnd, &mut r);
        let left = pt.x < (r.left + r.right) / 2;
        with_app(|a| a.drag = Some((left, r, pt)));
        SetCapture(hwnd);
    }
}

fn drag_resize(hwnd: HWND) {
    let Some(Some((left, r0, p0))) = with_app(|a| a.drag) else { return };
    unsafe {
        let mut pt: POINT = zeroed();
        GetCursorPos(&mut pt);
        let dx = pt.x - p0.x;
        let dpi = GetDpiForWindow(hwnd).max(96);
        let w = if left { r0.right - r0.left - dx } else { r0.right - r0.left + dx }.max(scale(MIN_W, dpi));
        with_app(|a| a.width = Some(w * 96 / dpi as i32));
        let (w, h) = pill_size(hwnd, dpi);
        let x = if left { r0.right - w } else { r0.left };
        SetWindowPos(hwnd, null_mut(), x, r0.top, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
        InvalidateRect(hwnd, null(), 0);
    }
}

fn end_resize(hwnd: HWND) {
    if with_app(|a| a.drag.take()).flatten().is_some() {
        save_settings();
        save_pos(hwnd);
    }
}

fn reset_size(hwnd: HWND) {
    with_app(|a| a.width = None);
    save_settings();
    resize_to_content(hwnd);
}

/// Grow or shrink the pill to fit its text, keeping the top-left corner where it is.
fn resize_to_content(hwnd: HWND) {
    unsafe {
        let mut r: RECT = zeroed();
        GetWindowRect(hwnd, &mut r);
        let (w, h) = pill_size(hwnd, GetDpiForWindow(hwnd).max(96));
        if (w, h) != (r.right - r.left, r.bottom - r.top) {
            fit_on_screen(hwnd, r.left, r.top);
        }
    }
}

fn update_stats_timer(hwnd: HWND) {
    let visible = unsafe { IsWindowVisible(hwnd) != 0 };
    let on = with_app(|a| {
        let run = a.sys_on.any() && visible;
        if !run {
            a.sampler.reset();
            a.sys = sysstats::Stats::default();
        }
        run
    })
    .unwrap_or(false);
    unsafe {
        if on {
            SetTimer(hwnd, STATS_TIMER, STATS_INTERVAL_MS, None);
        } else {
            KillTimer(hwnd, STATS_TIMER);
        }
    }
}

fn sample_stats(hwnd: HWND) {
    with_app(|a| a.sys = a.sampler.sample(a.sys_on));
    resize_to_content(hwnd);
    unsafe { InvalidateRect(hwnd, null(), 0) };
}

fn toggle_stat(hwnd: HWND, cmd: usize) {
    with_app(|a| match cmd {
        CMD_CPU => a.sys_on.cpu = !a.sys_on.cpu,
        CMD_RAM => a.sys_on.ram = !a.sys_on.ram,
        CMD_GPU => a.sys_on.gpu = !a.sys_on.gpu,
        _ => a.sys_on.net = !a.sys_on.net,
    });
    save_settings();
    update_stats_timer(hwnd);
    sample_stats(hwnd);
}

fn toggle_labels(hwnd: HWND) {
    if with_app(|a| a.labels = !a.labels).is_none() {
        return;
    }
    save_settings();
    unsafe {
        let mut r: RECT = zeroed();
        GetWindowRect(hwnd, &mut r);
        fit_on_screen(hwnd, r.left, r.top);
    }
    refresh(false);
}

fn pos_path() -> PathBuf {
    snapshot::data_dir().join(POS_FILE)
}

fn save_pos(hwnd: HWND) {
    unsafe {
        let mut r: RECT = zeroed();
        if GetWindowRect(hwnd, &mut r) != 0 {
            let _ = std::fs::write(pos_path(), format!("{} {}", r.left, r.top));
        }
    }
}

fn load_pos() -> Option<(i32, i32)> {
    let s = std::fs::read_to_string(pos_path()).ok()?;
    let mut it = s.split_whitespace().map(|v| v.parse::<i32>().ok());
    Some((it.next()??, it.next()??))
}

/// Size the pill for its monitor's DPI and keep it fully inside that monitor's work area.
fn fit_on_screen(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        SetWindowPos(hwnd, null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        let dpi = GetDpiForWindow(hwnd).max(96);
        let (w, h) = pill_size(hwnd, dpi);
        let r = RECT { left: x, top: y, right: x + w, bottom: y + h };
        let mon = MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = zeroed();
        mi.cbSize = size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(mon, &mut mi);
        let wa = mi.rcWork;
        let nx = x.clamp(wa.left, (wa.right - w).max(wa.left));
        let ny = y.clamp(wa.top, (wa.bottom - h).max(wa.top));
        SetWindowPos(hwnd, HWND_TOPMOST, nx, ny, w, h, SWP_NOACTIVATE);
    }
}

fn place_initial(hwnd: HWND) {
    let (x, y) = load_pos().unwrap_or_else(|| unsafe {
        let mut wa: RECT = zeroed();
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut wa as *mut _ as _, 0);
        let dpi = GetDpiForWindow(hwnd).max(96);
        let (w, h) = pill_size(hwnd, dpi);
        (wa.right - w - scale(16, dpi), wa.bottom - h - scale(16, dpi))
    });
    fit_on_screen(hwnd, x, y);
}

fn show_pill(hwnd: HWND, show: bool) {
    unsafe {
        if show {
            let mut r: RECT = zeroed();
            GetWindowRect(hwnd, &mut r);
            fit_on_screen(hwnd, r.left, r.top);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        } else {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
    update_stats_timer(hwnd);
}

fn toggle_pill(hwnd: HWND) {
    let visible = unsafe { IsWindowVisible(hwnd) != 0 };
    show_pill(hwnd, !visible);
}

fn show_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let visible = IsWindowVisible(hwnd) != 0;
        let m = CreatePopupMenu();
        let toggle = wide(if visible { "Hide pill" } else { "Show pill" });
        let labels = wide(if with_app(|a| a.labels).unwrap_or(true) { "Hide labels" } else { "Show labels" });
        let exit = wide("Exit TokenStat");
        AppendMenuW(m, MF_STRING, CMD_TOGGLE, toggle.as_ptr());
        AppendMenuW(m, MF_STRING, CMD_LABELS, labels.as_ptr());
        let auto_on = with_app(|a| a.auto_refresh).unwrap_or(false);
        let (count, tokens) = read_stats();
        let auto_text = wide(&if auto_on {
            format!("Auto-refresh after VS Code use ({count} today, {tokens} tokens)")
        } else {
            "Auto-refresh after VS Code use...".to_string()
        });
        AppendMenuW(m, MF_STRING | if auto_on { MF_CHECKED } else { MF_UNCHECKED }, CMD_AUTO, auto_text.as_ptr());
        let on = with_app(|a| a.sys_on).unwrap_or_default();
        let sub = CreatePopupMenu();
        let mut keep = Vec::new();
        for (cmd, label, checked) in [(CMD_CPU, "CPU", on.cpu), (CMD_RAM, "RAM", on.ram), (CMD_GPU, "GPU", on.gpu), (CMD_NET, "Network", on.net)] {
            let w = wide(label);
            AppendMenuW(sub, MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED }, cmd, w.as_ptr());
            keep.push(w);
        }
        let sys_label = wide("System stats");
        AppendMenuW(m, MF_POPUP, sub as usize, sys_label.as_ptr());
        let sized = with_app(|a| a.width.is_some()).unwrap_or(false);
        let reset = wide("Reset size (one row)");
        AppendMenuW(m, MF_STRING | if sized { 0 } else { MF_GRAYED }, CMD_RESET_SIZE, reset.as_ptr());
        let startup = wide("Start with Windows");
        AppendMenuW(m, MF_STRING | if startup_enabled() { MF_CHECKED } else { MF_UNCHECKED }, CMD_STARTUP, startup.as_ptr());
        AppendMenuW(m, MF_SEPARATOR, 0, null());
        AppendMenuW(m, MF_STRING, CMD_EXIT, exit.as_ptr());
        SetForegroundWindow(hwnd);
        TrackPopupMenuEx(m, TPM_RIGHTBUTTON, x, y, hwnd, null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(m);
    }
}

fn level_colour(pct: f64, normal: icon::Rgba) -> icon::Rgba {
    match view::level_of(pct) {
        view::Level::Red => icon::Rgba(255, 99, 97, 255),
        view::Level::Amber => icon::Rgba(255, 185, 0, 255),
        view::Level::Normal => normal,
    }
}

/// Live tray icon: outer ring weekly, inner ring session. Grey when the reading is stale.
fn make_tray_icon(rings: (Option<f64>, Option<f64>), freshness: view::Freshness) -> HICON {
    let (session, weekly) = rings;
    let grey = icon::Rgba(140, 140, 140, 255);
    let live = freshness == view::Freshness::Live;
    let pick = |p: Option<f64>, normal| if live { p.map(|v| level_colour(v, normal)).unwrap_or(grey) } else { grey };
    let spec = icon::Rings {
        outer_pct: weekly,
        inner_pct: session,
        outer: pick(weekly, icon::Rgba(76, 194, 111, 255)),
        inner: pick(session, icon::Rgba(240, 240, 240, 255)),
        track: icon::Rgba(58, 58, 58, 255),
        background: icon::Rgba(31, 31, 31, 255),
    };
    unsafe {
        let size = GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()).clamp(16, 64);
        let rgba = icon::render(size as usize, &spec);
        let mut bi: BITMAPINFO = zeroed();
        bi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = size;
        bi.bmiHeader.biHeight = -size;
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB;
        let mut bits: *mut core::ffi::c_void = null_mut();
        let color = CreateDIBSection(null_mut(), &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if color.is_null() || bits.is_null() {
            return LoadIconW(null_mut(), IDI_APPLICATION);
        }
        let dst = std::slice::from_raw_parts_mut(bits as *mut u8, rgba.len());
        for (d, s) in dst.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
            d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
        }
        let mask = CreateBitmap(size, size, 1, 1, null());
        let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let h = CreateIconIndirect(&info);
        DeleteObject(color as _);
        DeleteObject(mask as _);
        if h.is_null() {
            LoadIconW(null_mut(), IDI_APPLICATION)
        } else {
            h
        }
    }
}

struct Palette {
    bg: COLORREF,
    fg: COLORREF,
    live: COLORREF,
    stale: COLORREF,
    amber: COLORREF,
    red: COLORREF,
}

// Windows stores the app light/dark choice here; missing value means light (Windows default).
fn light_theme() -> bool {
    let key = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let name = wide("AppsUseLightTheme");
    let mut value: u32 = 1;
    let mut size = size_of::<u32>() as u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            &mut value as *mut _ as _,
            &mut size,
        );
    }
    value != 0
}

fn palette() -> Palette {
    if high_contrast() {
        let (bg, fg) = unsafe { (GetSysColor(COLOR_WINDOW), GetSysColor(COLOR_WINDOWTEXT)) };
        Palette { bg, fg, live: fg, stale: fg, amber: fg, red: fg }
    } else if light_theme() {
        Palette {
            bg: rgb(243, 243, 243),
            fg: rgb(26, 26, 26),
            live: rgb(16, 124, 16),
            stale: rgb(120, 120, 120),
            amber: rgb(157, 93, 0),
            red: rgb(196, 43, 28),
        }
    } else {
        Palette {
            bg: rgb(32, 32, 32),
            fg: rgb(240, 240, 240),
            live: rgb(76, 194, 111),
            stale: rgb(140, 140, 140),
            amber: rgb(255, 185, 0),
            red: rgb(255, 99, 97),
        }
    }
}

fn paint(hwnd: HWND) {
    let Some((items, freshness, font, dpi)) = with_app(|a| (pill_items(a), a.view.freshness, a.font, a.dpi))
    else {
        unsafe { ValidateRect(hwnd, null()) };
        return;
    };
    let p = palette();
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rc: RECT = zeroed();
        GetClientRect(hwnd, &mut rc);

        let brush = CreateSolidBrush(p.bg);
        FillRect(hdc, &rc, brush);
        DeleteObject(brush as _);

        let d = scale(8, dpi);
        let x = scale(12, dpi);
        let (row_h, pad_y) = (scale(ROW_H, dpi), scale(PAD_Y, dpi));
        let y = pad_y + (row_h - d) / 2;
        let dot = if freshness == view::Freshness::Live { p.live } else { p.stale };
        let dot_brush = CreateSolidBrush(dot);
        let old_brush = SelectObject(hdc, dot_brush as _);
        let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
        Ellipse(hdc, x, y, x + d + 1, y + d + 1);
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        DeleteObject(dot_brush as _);

        SetBkMode(hdc, TRANSPARENT as _);
        let old_font = SelectObject(hdc, font as _);
        let widths: Vec<i32> = items.iter().map(|i| line_width(hdc, i)).collect();
        let widest = widths.iter().copied().max().unwrap_or(0);
        let inner = (rc.right - scale(PAD_L + PAD_R, dpi)).max(widest);
        let rows = view::wrap(&widths, inner, scale(GAP, dpi));
        for (ri, row) in rows.iter().enumerate() {
            let row_top = pad_y + ri as i32 * row_h;
            let mut tx = scale(PAD_L, dpi);
            for &idx in row {
                let mut sx = tx;
                for (text, level) in &items[idx] {
                    let colour = match level {
                        Some(view::Level::Amber) => p.amber,
                        Some(view::Level::Red) => p.red,
                        _ => p.fg,
                    };
                    let w: Vec<u16> = text.encode_utf16().collect();
                    let mut sz: SIZE = zeroed();
                    GetTextExtentPoint32W(hdc, w.as_ptr(), w.len() as i32, &mut sz);
                    SetTextColor(hdc, colour);
                    TextOutW(hdc, sx, row_top + (row_h - sz.cy) / 2, w.as_ptr(), w.len() as i32);
                    sx += sz.cx;
                }
                tx += widths[idx] + scale(GAP, dpi);
            }
        }
        SelectObject(hdc, old_font);
        EndPaint(hwnd, &ps);
    }
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let taskbar_created = with_app(|a| a.taskbar_created).unwrap_or(0);
    if msg != 0 && msg == taskbar_created {
        if let Some((tip, icon)) = with_app(|a| (a.view.tooltip.clone(), a.tray_icon)) {
            add_tray(hwnd, &tip, icon);
        }
        return 0;
    }
    match msg {
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_NCHITTEST => {
            if resize_edge_hit(hwnd, signed_lo(lparam as usize)) {
                HTCLIENT as LRESULT
            } else {
                HTCAPTION as LRESULT
            }
        }
        WM_SETCURSOR if loword(lparam as usize) == HTCLIENT => {
            SetCursor(LoadCursorW(null_mut(), IDC_SIZEWE));
            1
        }
        WM_LBUTTONDOWN => {
            begin_resize(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            drag_resize(hwnd);
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            end_resize(hwnd);
            0
        }
        WM_CAPTURECHANGED => {
            end_resize(hwnd);
            0
        }
        WM_NCLBUTTONDBLCLK => {
            show_pill(hwnd, false);
            0
        }
        WM_NCRBUTTONUP => {
            show_menu(hwnd, signed_lo(lparam as usize), signed_hi(lparam as usize));
            0
        }
        WM_EXITSIZEMOVE => {
            save_pos(hwnd);
            0
        }
        WM_KEYDOWN if wparam as u16 == VK_ESCAPE => {
            show_pill(hwnd, false);
            0
        }
        WM_CLOSE => {
            show_pill(hwnd, false);
            0
        }
        WM_TIMER if wparam == STATS_TIMER => {
            sample_stats(hwnd);
            0
        }
        WM_REFRESH_DONE => {
            refresh_done(wparam != 0);
            0
        }
        WM_TRAY => {
            match loword(lparam as usize) {
                NIN_SELECT | NIN_KEYSELECT => toggle_pill(hwnd),
                WM_CONTEXTMENU => show_menu(hwnd, signed_lo(wparam), signed_hi(wparam)),
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            match loword(wparam) as usize {
                CMD_TOGGLE => toggle_pill(hwnd),
                CMD_LABELS => toggle_labels(hwnd),
                CMD_AUTO => toggle_auto_refresh(hwnd),
                CMD_STARTUP => toggle_startup(),
                CMD_RESET_SIZE => reset_size(hwnd),
                CMD_CPU | CMD_RAM | CMD_GPU | CMD_NET => toggle_stat(hwnd, loword(wparam) as usize),
                CMD_EXIT => {
                    DestroyWindow(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_DPICHANGED => {
            let dpi = ((wparam >> 16) & 0xFFFF) as u32;
            with_app(|a| {
                DeleteObject(a.font as _);
                a.font = make_font(dpi);
                a.dpi = dpi;
            });
            let r = &*(lparam as *const RECT);
            SetWindowPos(hwnd, null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
            InvalidateRect(hwnd, null(), 1);
            0
        }
        WM_DISPLAYCHANGE => {
            let mut r: RECT = zeroed();
            GetWindowRect(hwnd, &mut r);
            fit_on_screen(hwnd, r.left, r.top);
            0
        }
        WM_TIMECHANGE => {
            refresh(false);
            0
        }
        WM_POWERBROADCAST => {
            if wparam as u32 == PBT_APMRESUMEAUTOMATIC {
                refresh(false);
            }
            1
        }
        WM_SETTINGCHANGE | WM_SYSCOLORCHANGE => {
            InvalidateRect(hwnd, null(), 1);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_DESTROY => {
            let nid = tray_data(hwnd, "", null_mut());
            Shell_NotifyIconW(NIM_DELETE, &nid);
            if let Some(i) = with_app(|a| a.tray_icon) {
                DestroyIcon(i);
            }
            save_pos(hwnd);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn run() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mutex_name = wide("Local\\TokenStat.Singleton");
        let _mutex = CreateMutexW(null(), 1, mutex_name.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }

        let dir = snapshot::data_dir();
        let _ = std::fs::create_dir_all(&dir);

        let hinst = GetModuleHandleW(null());
        let class = wide("TokenStatPill");
        let title = wide("TokenStat");
        let mut wc: WNDCLASSEXW = zeroed();
        wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = hinst;
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.hIcon = LoadIconW(hinst, 1 as _);
        wc.lpszClassName = class.as_ptr();
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            BASE_W,
            BASE_H,
            null_mut(),
            null_mut(),
            hinst,
            null(),
        );
        if hwnd.is_null() {
            return;
        }
        let corner = DWMWCP_ROUND;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &corner as *const _ as _,
            size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );

        let dpi = GetDpiForWindow(hwnd).max(96);
        let labels = load_setting("labels", true);
        let timer = CreateWaitableTimerW(null(), 0, null());
        let t = now();
        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                hwnd,
                dir: dir.clone(),
                snap: None,
                load_error: false,
                view: view::build(None, false, t, labels, &|x| format_local(x, t)),
                font: make_font(dpi),
                dpi,
                timer,
                taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
                labels,
                tray_icon: null_mut(),
                auto_refresh: load_setting("refresh", false),
                refreshing: false,
                next_refresh_ok: 0,
                failures: 0,
                activity_at: None,
                sys_on: sysstats::Enabled {
                    cpu: load_setting("cpu", false),
                    ram: load_setting("ram", false),
                    gpu: load_setting("gpu", false),
                    net: load_setting("net", false),
                },
                sys: sysstats::Stats::default(),
                sampler: sysstats::Sampler::new(),
                width: load_number("width").filter(|w| *w > 0),
                drag: None,
            })
        });

        place_initial(hwnd);
        let first_icon = make_tray_icon((None, None), view::Freshness::Unknown);
        let tip = with_app(|a| {
            a.tray_icon = first_icon;
            a.view.tooltip.clone()
        })
        .unwrap_or_default();
        add_tray(hwnd, &tip, first_icon);
        refresh(true);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        update_stats_timer(hwnd);

        let dir_w = wide(&dir.to_string_lossy());
        let change = FindFirstChangeNotificationW(
            dir_w.as_ptr(),
            0,
            FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_FILE_NAME,
        );
        let watching = change != INVALID_HANDLE_VALUE;
        let handles: Vec<HANDLE> = if watching { vec![timer, change] } else { vec![timer] };

        loop {
            let r = MsgWaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, INFINITE, QS_ALLINPUT);
            if r == WAIT_OBJECT_0 {
                refresh(false);
            } else if watching && r == WAIT_OBJECT_0 + 1 {
                FindNextChangeNotification(change);
                refresh(true);
            } else if r == WAIT_OBJECT_0 + handles.len() as u32 {
                let mut msg: MSG = zeroed();
                while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if msg.message == WM_QUIT {
                        if watching {
                            FindCloseChangeNotification(change);
                        }
                        CloseHandle(timer);
                        return;
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            } else {
                return;
            }
        }
    }
}
