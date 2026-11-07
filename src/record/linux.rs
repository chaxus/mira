//! Linux screen recording — PipeWire via `wf-recorder` on Wayland, else ffmpeg `x11grab`.
//!
//! Encode uses the same trimmed/PATH ffmpeg side-car when remux/GIF is needed.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use super::{RecordError, RecordOptions};
use crate::ffmpeg;

pub fn start(options: &RecordOptions) -> Result<(&'static str, Child, PathBuf), RecordError> {
    let path = raw_path(options);
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var_os("XDG_SESSION_TYPE").is_some_and(|v| v == "wayland");

    if wayland {
        if which("wf-recorder") {
            // wf-recorder talks to the compositor over PipeWire / wlr-screencopy.
            return Ok((
                "wf-recorder (PipeWire)",
                start_wf_recorder(options, &path)?,
                path,
            ));
        }
        return Err(RecordError::Message(
            "Wayland record needs `wf-recorder` (PipeWire). Install it or use an X11 session with ffmpeg."
                .into(),
        ));
    }

    if std::env::var_os("DISPLAY").is_some() {
        let ff = ffmpeg::resolve().ok_or_else(|| {
            RecordError::Message(
                "linux record needs ffmpeg (PATH / vendor / MIRA_FFMPEG) for x11grab".into(),
            )
        })?;
        return Ok(("ffmpeg x11grab", start_x11grab(&ff, options, &path)?, path));
    }

    Err(RecordError::Message(
        "linux record: set DISPLAY for x11grab+ffmpeg, or install wf-recorder on Wayland".into(),
    ))
}

fn raw_path(options: &RecordOptions) -> PathBuf {
    let mut path = super::media_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ext = match options.format {
        super::RecordFormat::Mp4 | super::RecordFormat::Gif => "mp4",
    };
    path.push(format!("Mira-{stamp}.{ext}"));
    path
}

fn start_wf_recorder(options: &RecordOptions, path: &Path) -> Result<Child, RecordError> {
    let mut cmd = Command::new("wf-recorder");
    cmd.arg("-f").arg(path);
    if !options.show_cursor {
        // wf-recorder has no universal cursor flag across versions; best-effort.
        let _ = options.show_cursor;
    }
    if options.region {
        // Interactive geometry via slurp when available.
        if which("slurp") {
            let geo = Command::new("slurp")
                .arg("-f")
                .arg("%x,%y %wx%h")
                .output()
                .map_err(|e| RecordError::Message(format!("slurp: {e}")))?;
            if !geo.status.success() {
                return Err(RecordError::Message("region select cancelled".into()));
            }
            let geo = String::from_utf8_lossy(&geo.stdout).trim().to_string();
            if geo.is_empty() {
                return Err(RecordError::Message("region select cancelled".into()));
            }
            cmd.arg("-g").arg(geo);
        }
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(cmd.spawn()?)
}

fn start_x11grab(ffmpeg: &Path, options: &RecordOptions, path: &Path) -> Result<Child, RecordError> {
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0.0".into());
    let mut args = vec![
        "-y".into(),
        "-f".into(),
        "x11grab".into(),
        "-framerate".into(),
        "30".into(),
    ];
    if options.show_cursor {
        args.extend(["-draw_mouse".into(), "1".into()]);
    } else {
        args.extend(["-draw_mouse".into(), "0".into()]);
    }

    if options.region {
        if which("slurp") {
            let geo = Command::new("slurp")
                .arg("-f")
                .arg("%x,%y %wx%h")
                .output()
                .map_err(|e| RecordError::Message(format!("slurp: {e}")))?;
            if !geo.status.success() {
                return Err(RecordError::Message("region select cancelled".into()));
            }
            let geo = String::from_utf8_lossy(&geo.stdout).trim().to_string();
            // "x,y wxh"
            let (pos, size) = geo.split_once(' ').ok_or_else(|| {
                RecordError::Message(format!("unexpected slurp output: {geo}"))
            })?;
            let (w, h) = size.split_once('x').ok_or_else(|| {
                RecordError::Message(format!("unexpected slurp size: {size}"))
            })?;
            args.extend([
                "-video_size".into(),
                format!("{w}x{h}"),
                "-i".into(),
                format!("{display}+{pos}"),
            ]);
        } else {
            return Err(RecordError::Message(
                "region record needs `slurp` (or use fullscreen)".into(),
            ));
        }
    } else {
        args.extend(["-i".into(), display]);
    }

    args.extend([
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-movflags".into(),
        "+faststart".into(),
        path.to_string_lossy().into_owned(),
    ]);

    Ok(Command::new(ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?)
}

fn which(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
