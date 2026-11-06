//! Global hotkeys: region / fullscreen capture → clipboard, with remappable bindings.

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use parking_lot::Mutex;

use crate::capture::{self, CaptureMode};
use crate::export;

static STARTED: AtomicBool = AtomicBool::new(false);
static RELOAD: AtomicBool = AtomicBool::new(false);
static LAST_STATUS: Mutex<String> = Mutex::new(String::new());
static BINDINGS: Mutex<Option<HotkeyBindings>> = Mutex::new(None);

/// Remappable capture shortcuts (string form accepted by `global-hotkey`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyBindings {
    pub region: String,
    pub fullscreen: String,
}

impl Default for HotkeyBindings {
    fn default() -> Self {
        Self {
            region: "Ctrl+Shift+A".into(),
            fullscreen: "Ctrl+Shift+S".into(),
        }
    }
}

impl HotkeyBindings {
    fn parse(&self) -> Result<(HotKey, HotKey), String> {
        let region = HotKey::from_str(&self.region)
            .map_err(|e| format!("region hotkey '{}': {e}", self.region))?;
        let fullscreen = HotKey::from_str(&self.fullscreen)
            .map_err(|e| format!("fullscreen hotkey '{}': {e}", self.fullscreen))?;
        if region.id() == fullscreen.id() {
            return Err("region and fullscreen hotkeys must differ".into());
        }
        Ok((region, fullscreen))
    }

    fn validate(&self) -> Result<(), String> {
        self.parse().map(|_| ())
    }
}

/// Latest hotkey status line for the UI to show.
pub fn last_status() -> String {
    LAST_STATUS.lock().clone()
}

fn set_status(msg: impl Into<String>) {
    *LAST_STATUS.lock() = msg.into();
}

/// Currently active (or default) bindings for the settings UI.
pub fn current_bindings() -> HotkeyBindings {
    BINDINGS
        .lock()
        .clone()
        .unwrap_or_else(load_or_default)
}

/// Persist and apply new bindings. Returns an error if either chord is invalid.
pub fn apply_bindings(bindings: HotkeyBindings) -> Result<(), String> {
    bindings.validate()?;
    save_bindings(&bindings)?;
    *BINDINGS.lock() = Some(bindings.clone());
    RELOAD.store(true, Ordering::SeqCst);
    set_status(format!(
        "hotkeys updating · {} region · {} screen",
        bindings.region, bindings.fullscreen
    ));
    Ok(())
}

/// Start the background hotkey listener once.
pub fn ensure_started() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let initial = load_or_default();
    *BINDINGS.lock() = Some(initial.clone());
    thread::spawn(move || {
        if let Err(err) = run_loop(initial) {
            set_status(format!("hotkey unavailable: {err}"));
        }
    });
}

fn run_loop(initial: HotkeyBindings) -> Result<(), String> {
    let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
    let mut registered = initial.clone();
    let mut active = register_pair(&manager, &registered)?;
    set_status(format!(
        "hotkeys ready · {} region · {} screen",
        registered.region, registered.fullscreen
    ));

    loop {
        if RELOAD.swap(false, Ordering::SeqCst) {
            let next = BINDINGS
                .lock()
                .clone()
                .unwrap_or_else(HotkeyBindings::default);
            let _ = manager.unregister(active.0);
            let _ = manager.unregister(active.1);
            match register_pair(&manager, &next) {
                Ok(pair) => {
                    active = pair;
                    registered = next.clone();
                    set_status(format!(
                        "hotkeys ready · {} region · {} screen",
                        next.region, next.fullscreen
                    ));
                }
                Err(err) => {
                    // Restore the last working chords so capture is not left dead.
                    if let Ok(pair) = register_pair(&manager, &registered) {
                        active = pair;
                        *BINDINGS.lock() = Some(registered.clone());
                        let _ = save_bindings(&registered);
                    }
                    set_status(format!("hotkey reload failed: {err}"));
                }
            }
        }

        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state != HotKeyState::Pressed {
                continue;
            }
            let mode = if event.id == active.0.id() {
                CaptureMode::Region
            } else if event.id == active.1.id() {
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

fn register_pair(
    manager: &GlobalHotKeyManager,
    bindings: &HotkeyBindings,
) -> Result<(HotKey, HotKey), String> {
    let (region, fullscreen) = bindings.parse()?;
    manager.register(region).map_err(|e| e.to_string())?;
    if let Err(err) = manager.register(fullscreen) {
        let _ = manager.unregister(region);
        return Err(err.to_string());
    }
    Ok((region, fullscreen))
}

fn load_or_default() -> HotkeyBindings {
    load_bindings().unwrap_or_default()
}

fn config_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        #[cfg(target_os = "macos")]
        {
            return PathBuf::from(home)
                .join("Library/Application Support/mira/hotkeys.conf");
        }
        #[cfg(not(target_os = "macos"))]
        {
            return PathBuf::from(home).join(".config/mira/hotkeys.conf");
        }
    }
    std::env::temp_dir().join("mira-hotkeys.conf")
}

fn load_bindings() -> Option<HotkeyBindings> {
    let raw = fs::read_to_string(config_path()).ok()?;
    let mut region = None;
    let mut fullscreen = None;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "region" => region = Some(value.trim().to_string()),
            "fullscreen" => fullscreen = Some(value.trim().to_string()),
            _ => {}
        }
    }
    let bindings = HotkeyBindings {
        region: region.unwrap_or_else(|| HotkeyBindings::default().region),
        fullscreen: fullscreen.unwrap_or_else(|| HotkeyBindings::default().fullscreen),
    };
    bindings.validate().ok()?;
    Some(bindings)
}

fn save_bindings(bindings: &HotkeyBindings) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = format!(
        "# Mira hotkey bindings (Ctrl+Shift+A style)\nregion={}\nfullscreen={}\n",
        bindings.region.trim(),
        bindings.fullscreen.trim()
    );
    fs::write(&path, body).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_parse() {
        let b = HotkeyBindings::default();
        assert!(b.parse().is_ok());
    }

    #[test]
    fn rejects_identical_chords() {
        let b = HotkeyBindings {
            region: "Ctrl+Shift+A".into(),
            fullscreen: "Ctrl+Shift+A".into(),
        };
        assert!(b.validate().is_err());
    }

    #[test]
    fn rejects_garbage() {
        let b = HotkeyBindings {
            region: "not-a-key".into(),
            fullscreen: "Ctrl+Shift+S".into(),
        };
        assert!(b.validate().is_err());
    }
}
