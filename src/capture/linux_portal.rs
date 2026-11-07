//! Linux Wayland / PipeWire-oriented screenshot helpers.
//!
//! Prefer `grim` (wlr-screencopy / PipeWire on wlroots and many compositors).
//! Optional xdg-desktop-portal Screenshot via `gdbus` when grim is absent.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use super::CaptureError;

/// Fullscreen capture on Wayland-class sessions.
pub fn capture_fullscreen_png(path: &Path, show_cursor: bool) -> Result<(), CaptureError> {
    if which("grim") {
        return grim_fullscreen(path, show_cursor);
    }
    portal_screenshot(path)
}

fn grim_fullscreen(path: &Path, show_cursor: bool) -> Result<(), CaptureError> {
    let mut cmd = Command::new("grim");
    if show_cursor {
        cmd.arg("-c");
    }
    cmd.arg(path);
    let status = cmd
        .status()
        .map_err(|e| CaptureError::Message(format!("grim: {e}")))?;
    if status.success() && path.exists() {
        return Ok(());
    }
    Err(CaptureError::Message(
        "grim fullscreen failed (is a Wayland compositor running?)".into(),
    ))
}

/// Best-effort portal Screenshot (non-interactive). Requires `gdbus` + a
/// desktop portal implementation. Cancels / timeouts surface as errors so
/// callers can fall back to `xcap`.
fn portal_screenshot(path: &Path) -> Result<(), CaptureError> {
    if !which("gdbus") {
        return Err(CaptureError::Message("no grim / gdbus for portal capture".into()));
    }

    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.portal.Screenshot.Screenshot",
            "",
            "{'interactive': <false>, 'modal': <false>}",
        ])
        .output()
        .map_err(|e| CaptureError::Message(format!("portal: {e}")))?;
    if !output.status.success() {
        return Err(CaptureError::Message(format!(
            "portal screenshot failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let request = parse_object_path(&stdout).ok_or_else(|| {
        CaptureError::Message(format!("unexpected portal reply: {stdout}"))
    })?;

    let uri = wait_for_uri(&request)?;
    copy_uri_to_path(&uri, path)
}

fn wait_for_uri(request: &str) -> Result<String, CaptureError> {
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline {
        // `gdbus monitor` exits when the process is killed; use a short timeout.
        let mut child = Command::new("gdbus")
            .args([
                "monitor",
                "--session",
                "--dest",
                "org.freedesktop.portal.Desktop",
                "--object-path",
                request,
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::Message(format!("gdbus monitor: {e}")))?;

        std::thread::sleep(Duration::from_millis(800));
        let _ = child.kill();
        if let Ok(out) = child.wait_with_output() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(uri) = parse_uri_from_response(&text) {
                return Ok(uri);
            }
            if text.contains("uint32 1") {
                return Err(CaptureError::Cancelled);
            }
        }
    }
    Err(CaptureError::Message(
        "timed out waiting for portal screenshot".into(),
    ))
}

fn parse_object_path(reply: &str) -> Option<String> {
    let start = reply.find("/org/")?;
    let rest = &reply[start..];
    let end = rest
        .find(|c: char| c == '\'' || c == '"' || c.is_whitespace() || c == ',')
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn parse_uri_from_response(text: &str) -> Option<String> {
    for marker in ["file://", "FILE://"] {
        if let Some(idx) = text.find(marker) {
            let rest = &text[idx..];
            let end = rest
                .find(|c: char| c == '\'' || c == '"' || c == '>' || c.is_whitespace())
                .unwrap_or(rest.len());
            let uri = &rest[..end];
            if uri.len() > marker.len() {
                return Some(uri.to_string());
            }
        }
    }
    None
}

fn copy_uri_to_path(uri: &str, path: &Path) -> Result<(), CaptureError> {
    let src = uri
        .strip_prefix("file://")
        .or_else(|| uri.strip_prefix("FILE://"))
        .unwrap_or(uri);
    let decoded = percent_decode(src);
    std::fs::copy(&decoded, path).map_err(|e| CaptureError::Message(format!("portal copy: {e}")))?;
    let _ = std::fs::remove_file(&decoded);
    Ok(())
}

fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4 | l) as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn which(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_object_path() {
        let reply = "(objectpath '/org/freedesktop/portal/desktop/request/1_0',)";
        assert_eq!(
            parse_object_path(reply).as_deref(),
            Some("/org/freedesktop/portal/desktop/request/1_0")
        );
    }

    #[test]
    fn parses_file_uri() {
        let text = "uri': <'file:///tmp/shot.png'>";
        assert_eq!(
            parse_uri_from_response(text).as_deref(),
            Some("file:///tmp/shot.png")
        );
    }

    #[test]
    fn percent_decode_spaces() {
        assert_eq!(percent_decode("/tmp/my%20shot.png"), "/tmp/my shot.png");
    }
}
