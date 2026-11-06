//! Global hotkeys (P1): region capture → clipboard, Lark-like defaults.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use parking_lot::Mutex;

use crate::capture::{self, CaptureMode};
use crate::export;

static STARTED: AtomicBool = AtomicBool::new(false);
static LAST_STATUS: Mutex<String> = Mutex::new(String::new());

/// Latest hotkey status line for the UI to show.
pub fn last_status() -> String {
    LAST_STATUS.lock().clone()
}

fn set_status(msg: impl Into<String>) {
    *LAST_STATUS.lock() = msg.into();
}

/// Start the background hotkey listener once.
///
/// Defaults (Lark-like):
/// - macOS / others: Ctrl+Shift+A → region capture → clipboard
/// - Ctrl+Shift+S → fullscreen → clipboard
pub fn ensure_started() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    thread::spawn(|| {
        if let Err(err) = run_loop() {
            set_status(format!("hotkey unavailable: {err}"));
        }
    });
}

fn run_loop() -> Result<(), String> {
    let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
    // Modifiers::CONTROL | SHIFT works on macOS too (Lark Win defaults);
    // users can remap later. Meta+Shift+A often conflicts with system.
    let region = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyA);
    let full = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyS);
    manager.register(region).map_err(|e| e.to_string())?;
    manager.register(full).map_err(|e| e.to_string())?;
    set_status("hotkeys ready · Ctrl+Shift+A region · Ctrl+Shift+S screen");

    let region_id = region.id();
    let full_id = full.id();
    loop {
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state != HotKeyState::Pressed {
                continue;
            }
            let mode = if event.id == region_id {
                CaptureMode::Region
            } else if event.id == full_id {
                CaptureMode::Fullscreen
            } else {
                continue;
            };
            match capture::capture(mode) {
                Ok(img) => match export::copy_to_clipboard(&img) {
                    Ok(()) => set_status(format!(
                        "captured {}×{} → clipboard",
                        img.width(),
                        img.height()
                    )),
                    Err(e) => set_status(format!("clipboard failed: {e}")),
                },
                Err(capture::CaptureError::Cancelled) => set_status("capture cancelled"),
                Err(e) => set_status(format!("capture failed: {e}")),
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}
