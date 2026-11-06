//! Resolve a trimmed (or fallback) ffmpeg side-car for encode / remux / GIF.
//!
//! Lookup order (see `docs/TECH_PLAN.md` option B):
//! 1. `MIRA_FFMPEG` env  
//! 2. Bundled `vendor/ffmpeg/<os>-<arch>/ffmpeg` (next to the binary, then crate root)  
//! 3. `ffmpeg` on `PATH`

use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FfmpegError {
    #[error("ffmpeg not found (set MIRA_FFMPEG, place vendor binary, or install on PATH)")]
    NotFound,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

/// Host triple folder under `vendor/ffmpeg/`, e.g. `macos-aarch64`.
pub fn vendor_triple() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "macos-aarch64"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "macos-x86_64"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "windows-x86_64"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "windows-aarch64"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "linux-x86_64"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "linux-aarch64"
    }
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    )))]
    {
        "unknown"
    }
}

fn ffmpeg_bin_name() -> &'static str {
    if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn vendor_candidates() -> Vec<PathBuf> {
    let triple = vendor_triple();
    let bin = ffmpeg_bin_name();
    let mut out = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("ffmpeg").join(triple).join(bin));
            out.push(dir.join("vendor").join("ffmpeg").join(triple).join(bin));
            // When running from `target/debug`, walk up to the crate root.
            out.push(
                dir.join("..")
                    .join("..")
                    .join("vendor")
                    .join("ffmpeg")
                    .join(triple)
                    .join(bin),
            );
        }
    }

    if let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR") {
        out.push(
            PathBuf::from(manifest)
                .join("vendor")
                .join("ffmpeg")
                .join(triple)
                .join(bin),
        );
    }

    // Dev convenience: repo-relative path from cwd.
    out.push(
        PathBuf::from("vendor")
            .join("ffmpeg")
            .join(triple)
            .join(bin),
    );

    out
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let with_exe = dir.join(format!("{name}.exe"));
            if with_exe.is_file() {
                return Some(with_exe);
            }
        }
    }
    None
}

/// Absolute path to an ffmpeg binary, if any resolution strategy succeeds.
pub fn resolve() -> Option<PathBuf> {
    if let Some(custom) = std::env::var_os("MIRA_FFMPEG") {
        let path = PathBuf::from(custom);
        if is_executable(&path) {
            return Some(path);
        }
    }

    for candidate in vendor_candidates() {
        if let Ok(canon) = candidate.canonicalize() {
            if is_executable(&canon) {
                return Some(canon);
            }
        } else if is_executable(&candidate) {
            return Some(candidate);
        }
    }

    which_on_path(ffmpeg_bin_name())
}

/// Whether any ffmpeg is available for encode / remux / GIF.
pub fn available() -> bool {
    resolve().is_some()
}

/// `Command` pointed at the resolved ffmpeg binary.
pub fn command() -> Result<Command, FfmpegError> {
    let path = resolve().ok_or(FfmpegError::NotFound)?;
    Ok(Command::new(path))
}

/// Remux (copy streams when possible) `input` → `output` as mp4.
pub fn remux_mp4(input: &Path, output: &Path) -> Result<(), FfmpegError> {
    let status = command()?
        .args([
            "-y",
            "-i",
            &input.to_string_lossy(),
            "-c",
            "copy",
            "-movflags",
            "+faststart",
            &output.to_string_lossy(),
        ])
        .status()?;
    if status.success() && output.exists() {
        return Ok(());
    }
    // Some .mov captures need a light re-encode for mp4.
    let status = command()?
        .args([
            "-y",
            "-i",
            &input.to_string_lossy(),
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            &output.to_string_lossy(),
        ])
        .status()?;
    if status.success() && output.exists() {
        Ok(())
    } else {
        Err(FfmpegError::Message(
            "ffmpeg failed to produce mp4 (is libx264 enabled in this build?)".into(),
        ))
    }
}

/// Convert a recording to an animated GIF (palettegen for acceptable quality).
pub fn to_gif(input: &Path, output: &Path) -> Result<(), FfmpegError> {
    let filter = "fps=12,scale=720:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse";
    let status = command()?
        .args([
            "-y",
            "-i",
            &input.to_string_lossy(),
            "-vf",
            filter,
            "-loop",
            "0",
            &output.to_string_lossy(),
        ])
        .status()?;
    if status.success() && output.exists() {
        Ok(())
    } else {
        Err(FfmpegError::Message(
            "ffmpeg failed to produce gif".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_triple_is_known() {
        assert_ne!(vendor_triple(), "unknown");
    }

    #[test]
    fn resolve_prefers_mira_ffmpeg_env() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join(ffmpeg_bin_name());
        std::fs::write(&fake, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&fake).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&fake, perms).unwrap();
        }

        // SAFETY: test-only; single-threaded unit test.
        unsafe {
            std::env::set_var("MIRA_FFMPEG", &fake);
        }
        let resolved = resolve();
        unsafe {
            std::env::remove_var("MIRA_FFMPEG");
        }
        assert_eq!(resolved.as_deref(), Some(fake.as_path()));
    }
}
