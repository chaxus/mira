//! System tray / menu-bar icon: Show · Region · Fullscreen · Quit.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

use crate::capture::{self, CaptureMode};
use crate::export;
use crate::hotkey;

static STARTED: AtomicBool = AtomicBool::new(false);
/// Menu ids are `Send`; the `TrayIcon` itself stays on the main thread (leaked).
static MENU: Mutex<Option<MenuIds>> = Mutex::new(None);

struct MenuIds {
    show: MenuId,
    region: MenuId,
    full: MenuId,
    quit: MenuId,
}

/// Create the tray on the **main thread** (required on macOS) and start the
/// action listener. Safe to call more than once.
pub fn ensure_started() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    match build_tray() {
        Ok(ids) => {
            *MENU.lock() = Some(ids);
            hotkey::note_status("tray ready · Show / Region / Fullscreen / Quit");
            thread::spawn(|| listen_loop());
        }
        Err(err) => {
            hotkey::note_status(format!("tray unavailable: {err}"));
            STARTED.store(false, Ordering::SeqCst);
        }
    }
}

fn build_tray() -> Result<MenuIds, String> {
    let show = MenuItem::new("Show Mira", true, None);
    let region = MenuItem::new("Capture Region", true, None);
    let full = MenuItem::new("Capture Fullscreen", true, None);
    let quit = MenuItem::new("Quit Mira", true, None);

    let ids = MenuIds {
        show: show.id().clone(),
        region: region.id().clone(),
        full: full.id().clone(),
        quit: quit.id().clone(),
    };

    let menu = Menu::new();
    menu.append(&show).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&region).map_err(|e| e.to_string())?;
    menu.append(&full).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&quit).map_err(|e| e.to_string())?;

    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Mira — screenshot & record")
        .with_icon(make_icon())
        .with_title("Mira")
        .build()
        .map_err(|e| e.to_string())?;

    // Process-lifetime: TrayIcon is !Send on macOS; keep it on this (main) thread.
    std::mem::forget(icon);
    std::mem::forget(show);
    std::mem::forget(region);
    std::mem::forget(full);
    std::mem::forget(quit);

    Ok(ids)
}

fn listen_loop() {
    loop {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let action = {
                let guard = MENU.lock();
                let Some(ids) = guard.as_ref() else {
                    continue;
                };
                if event.id == ids.show {
                    Some(Action::Show)
                } else if event.id == ids.region {
                    Some(Action::Region)
                } else if event.id == ids.full {
                    Some(Action::Fullscreen)
                } else if event.id == ids.quit {
                    Some(Action::Quit)
                } else {
                    None
                }
            };
            match action {
                Some(Action::Show) => show_panel(),
                Some(Action::Region) => run_capture(CaptureMode::Region),
                Some(Action::Fullscreen) => run_capture(CaptureMode::Fullscreen),
                Some(Action::Quit) => quit_app(),
                None => {}
            }
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
            ) {
                show_panel();
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}

enum Action {
    Show,
    Region,
    Fullscreen,
    Quit,
}

fn run_capture(mode: CaptureMode) {
    match capture::capture(mode) {
        Ok(img) => match export::copy_to_clipboard(&img) {
            Ok(()) => hotkey::note_status(format!(
                "tray captured {}×{} → clipboard",
                img.width(),
                img.height()
            )),
            Err(e) => hotkey::note_status(format!("clipboard failed: {e}")),
        },
        Err(capture::CaptureError::Cancelled) => hotkey::note_status("capture cancelled"),
        Err(e) => hotkey::note_status(format!("capture failed: {e}")),
    }
}

fn show_panel() {
    #[cfg(target_os = "macos")]
    {
        use cidre::ns;
        let mut app = ns::App::shared();
        app.activate_ignoring_other_apps(true);
        if let Some(mut window) = app.main_window() {
            window.make_key_and_order_front(None);
        }
        hotkey::note_status("show Mira");
    }
    #[cfg(not(target_os = "macos"))]
    {
        hotkey::note_status("show Mira — use the taskbar / dock if the window is hidden");
    }
}

fn quit_app() {
    hotkey::note_status("quitting");
    #[cfg(target_os = "macos")]
    {
        use cidre::ns;
        ns::App::shared().terminate(None);
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::process::exit(0);
    }
}

/// Simple template-friendly glyph: filled rounded square with a cutout “M”.
fn make_icon() -> Icon {
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let i = ((y * SIZE + x) * 4) as usize;
            let edge = x.min(y).min(SIZE - 1 - x).min(SIZE - 1 - y);
            let in_body = edge >= 2;
            let leg = x <= 6 || x >= 25;
            let dx = (x as i32 - 16).unsigned_abs();
            let peak = dx <= (14u32.saturating_sub(y / 2)) && y < 18;
            let is_m = in_body && (leg || peak);
            if is_m {
                rgba[i] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            } else if in_body {
                rgba[i] = 20;
                rgba[i + 1] = 120;
                rgba[i + 2] = 200;
                rgba[i + 3] = 255;
            }
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("static tray icon")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_builds() {
        let _ = make_icon();
    }
}
