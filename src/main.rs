//! Runs Mira on the machine's native NSH backend.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn main() -> impl std::process::Termination {
    env_logger::init();
    platform::run_app(
        platform::AppSource::Rust(platform::Root::of::<mira::App>()),
        platform::AppOptions {
            title: "Mira".to_owned(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: "com.chaxus.mira".to_owned(),
            size: (440, 720),
            // Close hides the panel; reopen from Dock / task switcher. Hotkeys keep working.
            #[cfg(target_os = "macos")]
            keep_alive_on_close: true,
            ..Default::default()
        },
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!(
    "Run this binary on macOS, Windows or Linux; mobile targets come later via nsh run."
);
