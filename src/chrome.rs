//! Hide Mira's own windows while capturing (Lark Shift-to-hide behaviour).

use std::time::Duration;

/// RAII guard: hides the app on create, restores on drop.
pub struct HideGuard {
    active: bool,
}

impl HideGuard {
    /// Hide chrome, wait briefly so the compositor settles, then return the guard.
    pub fn acquire() -> Self {
        let active = hide_app();
        if active {
            std::thread::sleep(Duration::from_millis(150));
        }
        Self { active }
    }
}

impl Drop for HideGuard {
    fn drop(&mut self) {
        if self.active {
            show_app();
        }
    }
}

#[cfg(target_os = "macos")]
fn hide_app() -> bool {
    let pid = std::process::id();
    std::process::Command::new("osascript")
        .args([
            "-e",
            &format!(
                r#"tell application "System Events" to set visible of (first process whose unix id is {pid}) to false"#
            ),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn show_app() {
    let pid = std::process::id();
    let _ = std::process::Command::new("osascript")
        .args([
            "-e",
            &format!(
                r#"tell application "System Events" to set visible of (first process whose unix id is {pid}) to true"#
            ),
        ])
        .status();
}

#[cfg(target_os = "windows")]
fn hide_app() -> bool {
    windows_set_visible(false)
}

#[cfg(target_os = "windows")]
fn show_app() {
    let _ = windows_set_visible(true);
}

#[cfg(target_os = "windows")]
fn windows_set_visible(visible: bool) -> bool {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, SW_HIDE, SW_SHOW, ShowWindow,
    };

    struct Ctx {
        pid: u32,
        cmd: i32,
        hit: bool,
    }

    let mut ctx = Ctx {
        pid: std::process::id(),
        cmd: if visible { SW_SHOW } else { SW_HIDE },
        hit: false,
    };

    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = unsafe { &mut *(lparam as *mut Ctx) };
        let mut win_pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut win_pid);
            if win_pid == ctx.pid {
                ShowWindow(hwnd, ctx.cmd);
                ctx.hit = true;
            }
        }
        TRUE
    }

    let _ = unsafe { EnumWindows(Some(enum_cb), &mut ctx as *mut Ctx as LPARAM) };
    ctx.hit
}

#[cfg(target_os = "linux")]
fn hide_app() -> bool {
    let pid = std::process::id().to_string();
    std::process::Command::new("sh")
        .args([
            "-c",
            &format!(
                "command -v xdotool >/dev/null && xdotool search --pid {pid} windowminimize %@ || true"
            ),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn show_app() {
    let pid = std::process::id().to_string();
    let _ = std::process::Command::new("sh")
        .args([
            "-c",
            &format!(
                "command -v xdotool >/dev/null && xdotool search --pid {pid} windowactivate %@ windowraise %@ || true"
            ),
        ])
        .status();
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn hide_app() -> bool {
    false
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn show_app() {}
