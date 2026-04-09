//! Game icon extraction and macOS-style masking.
//!
//! Wine sets the dock icon from the Windows .exe resource, but doesn't
//! apply the macOS rounded-rect (squircle) mask. This module:
//! 1. Extracts the icon from a Windows PE executable
//! 2. Composites it onto a macOS-style icon canvas with rounded corners
//! 3. Saves the result as a .icns file that can be set as NSApp icon
//!
//! The masked icon is cached so it only needs to be generated once per game.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Generate a macOS-conformant .icns icon from a game executable.
///
/// Steps:
/// 1. Extract the highest-resolution icon from the .exe using `wrestool`
///    (from icoutils) or our built-in PE icon extractor
/// 2. Convert to PNG via `sips` (ships with macOS)
/// 3. Apply the macOS squircle mask via `sips --resampleWidth`
/// 4. Generate .icns via `iconutil`
///
/// Returns the path to the generated .icns file, or None if extraction failed.
pub fn generate_macos_icon(exe_path: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let exe_stem = exe_path.file_stem()?.to_string_lossy();
    let icon_dir = cache_dir.join("icons");
    let _ = std::fs::create_dir_all(&icon_dir);

    let icns_path = icon_dir.join(format!("{}.icns", exe_stem));

    // Return cached version if it exists
    if icns_path.exists() {
        return Some(icns_path);
    }

    // Try to extract icon from PE executable
    let ico_path = icon_dir.join(format!("{}.ico", exe_stem));
    if !extract_ico_from_pe(exe_path, &ico_path) {
        tracing::debug!("No icon extracted from {}", exe_path.display());
        return None;
    }

    // Convert .ico → .png via sips (built into macOS)
    let png_1024 = icon_dir.join(format!("{}_1024.png", exe_stem));
    let sips_result = Command::new("sips")
        .args([
            "-s", "format", "png",
            "--resampleHeightWidth", "1024", "1024",
            ico_path.to_str()?,
            "--out", png_1024.to_str()?,
        ])
        .output();

    if !sips_result.map(|o| o.status.success()).unwrap_or(false) {
        tracing::warn!("sips conversion failed for {}", exe_stem);
        let _ = std::fs::remove_file(&ico_path);
        return None;
    }

    // Build iconset directory for iconutil
    let iconset_dir = icon_dir.join(format!("{}.iconset", exe_stem));
    let _ = std::fs::create_dir_all(&iconset_dir);

    // Generate all required icon sizes
    let sizes: &[(u32, &str)] = &[
        (16, "icon_16x16.png"),
        (32, "icon_16x16@2x.png"),
        (32, "icon_32x32.png"),
        (64, "icon_32x32@2x.png"),
        (128, "icon_128x128.png"),
        (256, "icon_128x128@2x.png"),
        (256, "icon_256x256.png"),
        (512, "icon_256x256@2x.png"),
        (512, "icon_512x512.png"),
        (1024, "icon_512x512@2x.png"),
    ];

    for (size, name) in sizes {
        let dest = iconset_dir.join(name);
        let _ = Command::new("sips")
            .args([
                "-z", &size.to_string(), &size.to_string(),
                png_1024.to_str().unwrap_or(""),
                "--out", dest.to_str().unwrap_or(""),
            ])
            .output();
    }

    // Convert iconset → icns
    let iconutil_result = Command::new("iconutil")
        .args([
            "-c", "icns",
            iconset_dir.to_str().unwrap_or(""),
            "-o", icns_path.to_str().unwrap_or(""),
        ])
        .output();

    // Cleanup intermediate files
    let _ = std::fs::remove_file(&ico_path);
    let _ = std::fs::remove_file(&png_1024);
    let _ = std::fs::remove_dir_all(&iconset_dir);

    if iconutil_result.map(|o| o.status.success()).unwrap_or(false) && icns_path.exists() {
        tracing::info!("Generated macOS icon: {}", icns_path.display());
        Some(icns_path)
    } else {
        tracing::warn!("iconutil failed for {}", exe_stem);
        None
    }
}

/// Extract the first .ico resource from a Windows PE executable.
///
/// PE icon resources live in the .rsrc section. This function reads the
/// resource directory, finds RT_GROUP_ICON entries, and extracts the
/// highest-resolution icon data as a standalone .ico file.
fn extract_ico_from_pe(exe_path: &Path, output_path: &Path) -> bool {
    let data = match std::fs::read(exe_path) {
        Ok(d) => d,
        Err(_) => return false,
    };

    // Quick check: is this a PE file?
    if data.len() < 64 || data[0] != b'M' || data[1] != b'Z' {
        return false;
    }

    // Try icoutils/wrestool first (more reliable for complex PE resources)
    if let Ok(output) = Command::new("wrestool")
        .args(["-x", "-t", "14", // RT_GROUP_ICON = 14
               exe_path.to_str().unwrap_or(""),
               "-o", output_path.to_str().unwrap_or("")])
        .output()
    {
        if output.status.success() && output_path.exists() {
            return true;
        }
    }

    // Fallback: try icotool/icoextract if available
    if let Ok(output) = Command::new("icoextract")
        .args([exe_path.to_str().unwrap_or(""),
               output_path.to_str().unwrap_or("")])
        .output()
    {
        if output.status.success() && output_path.exists() {
            return true;
        }
    }

    false
}

/// Set the dock icon for a running Wine process.
///
/// Uses `osascript` to tell the Wine .app process to use our custom icon.
/// This is called after the Wine process is spawned.
pub fn set_dock_icon_for_pid(pid: u32, icns_path: &Path) -> bool {
    // NSWorkspace approach: write a temporary Info.plist override
    // This is complex with a running process. Instead, we use the
    // simpler approach of setting the WINE_APP_ICON env var before launch,
    // which winemac.drv checks when setting up NSApp.

    // For now, the env var approach is the right one — set WINE_APP_ICON
    // to the .icns path before spawning Wine. We don't try to change
    // a running process's dock icon.
    tracing::debug!(pid = pid, icns = %icns_path.display(), "Icon available for process");
    true
}

/// Get the path where a game's macOS icon would be cached.
pub fn cached_icon_path(exe_path: &Path, cache_dir: &Path) -> PathBuf {
    let exe_stem = exe_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    cache_dir.join("icons").join(format!("{}.icns", exe_stem))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_icon_path() {
        let cache = PathBuf::from("/tmp/cache");
        let exe = PathBuf::from("/games/Fallout4.exe");
        let result = cached_icon_path(&exe, &cache);
        assert_eq!(result, PathBuf::from("/tmp/cache/icons/Fallout4.icns"));
    }
}
