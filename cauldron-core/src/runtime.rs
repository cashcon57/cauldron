use std::fmt;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Runtime not found: {0}")]
    NotFound(String),
    #[error("Missing DLL in runtime distribution: {0}")]
    MissingDll(String),
    #[error("Invalid bottle path: {0}")]
    InvalidBottle(String),
}

/// The type of graphics runtime being managed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeType {
    /// DXVK — DirectX 9/10/11 to Vulkan translation.
    Dxvk,
    /// DXMT — DirectX 10/11 to Metal translation.
    Dxmt,
    /// MoltenVK — Vulkan-to-Metal ICD.
    MoltenVK,
    /// D3DMetal — Apple's Game Porting Toolkit D3D translation.
    D3DMetal,
}

impl fmt::Display for RuntimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dxvk => write!(f, "DXVK"),
            Self::Dxmt => write!(f, "DXMT"),
            Self::MoltenVK => write!(f, "MoltenVK"),
            Self::D3DMetal => write!(f, "D3DMetal"),
        }
    }
}

/// A specific version of a graphics runtime available on disk.
#[derive(Debug, Clone)]
pub struct RuntimeVersion {
    /// Human-readable name, e.g. "dxvk-1.10.3".
    pub name: String,
    /// The type of runtime this version provides.
    pub runtime_type: RuntimeType,
    /// Semver-ish version string, e.g. "1.10.3".
    pub version: String,
    /// Path to the extracted runtime distribution directory.
    pub path: PathBuf,
    /// Whether this runtime is currently installed in any bottle.
    pub installed: bool,
}

/// Manages downloading, installing, and removing graphics runtimes in Wine bottles.
pub struct RuntimeInstaller {
    /// Base directory where extracted runtime distributions live.
    pub runtimes_dir: PathBuf,
}

impl RuntimeInstaller {
    /// Create a new installer rooted at the given base directory.
    ///
    /// Runtimes are expected to live under `base_dir/runtimes/`.
    pub fn new(base_dir: PathBuf) -> Self {
        let runtimes_dir = base_dir.join("runtimes");
        tracing::debug!(runtimes_dir = %runtimes_dir.display(), "RuntimeInstaller initialized");
        Self { runtimes_dir }
    }

    /// Install a runtime into the given Wine bottle.
    ///
    /// Copies the appropriate DLLs into the bottle's `system32/` (and `syswow64/`
    /// for 32-bit support if the directory exists), then writes Wine DLL override
    /// registry entries so Wine uses the native versions.
    pub fn install_to_bottle(
        &self,
        runtime: &RuntimeVersion,
        bottle_path: &Path,
    ) -> Result<(), RuntimeError> {
        let system32 = bottle_path.join("drive_c/windows/system32");
        if !system32.exists() {
            tracing::error!(path = %system32.display(), "system32 directory not found in bottle");
            return Err(RuntimeError::InvalidBottle(format!(
                "system32 not found at {}",
                system32.display()
            )));
        }

        let syswow64 = bottle_path.join("drive_c/windows/syswow64");
        let has_wow64 = syswow64.exists();

        tracing::info!(
            "Installing {} ({}) into bottle at {}",
            runtime.name,
            runtime.runtime_type,
            bottle_path.display()
        );

        match runtime.runtime_type {
            RuntimeType::Dxvk => {
                let dlls = ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"];
                let x64_dir = runtime.path.join("x64");
                let x32_dir = runtime.path.join("x32");

                for dll in &dlls {
                    copy_dll(&x64_dir, &system32, dll)?;
                }

                if has_wow64 && x32_dir.exists() {
                    for dll in &dlls {
                        copy_dll(&x32_dir, &syswow64, dll)?;
                    }
                }

                Self::write_dll_overrides(bottle_path, &dlls, "native")?;
            }
            RuntimeType::Dxmt => {
                // DXMT does not handle DX9.
                let dlls = ["d3d10core.dll", "d3d11.dll", "dxgi.dll"];
                let x64_dir = runtime.path.join("x64");

                for dll in &dlls {
                    copy_dll(&x64_dir, &system32, dll)?;
                }

                // DXMT is Metal-native; typically no 32-bit path.
                Self::write_dll_overrides(bottle_path, &dlls, "native")?;
            }
            RuntimeType::MoltenVK => {
                let src = runtime.path.join("libMoltenVK.dylib");
                if !src.exists() {
                    return Err(RuntimeError::MissingDll(format!(
                        "libMoltenVK.dylib not found in {}",
                        runtime.path.display()
                    )));
                }

                // MoltenVK goes into the bottle's lib directory so the Vulkan
                // loader can find it at runtime.
                let lib_dir = bottle_path.join("lib");
                fs::create_dir_all(&lib_dir)?;
                let dest = lib_dir.join("libMoltenVK.dylib");
                fs::copy(&src, &dest)?;
                tracing::debug!("Copied libMoltenVK.dylib -> {}", dest.display());

                // No DLL overrides needed; MoltenVK is a Vulkan ICD, not a
                // Windows DLL.
            }
            RuntimeType::D3DMetal => {
                // D3DMetal ships d3d11.dll and dxgi.dll (plus libd3dshared.dylib).
                let dlls = ["d3d11.dll", "dxgi.dll"];

                for dll in &dlls {
                    copy_dll(&runtime.path, &system32, dll)?;
                }

                // Copy the shared dylib if present.
                let shared_lib = runtime.path.join("libd3dshared.dylib");
                if shared_lib.exists() {
                    let lib_dir = bottle_path.join("lib");
                    fs::create_dir_all(&lib_dir)?;
                    fs::copy(&shared_lib, lib_dir.join("libd3dshared.dylib"))?;
                }

                Self::write_dll_overrides(bottle_path, &dlls, "native")?;
            }
        }

        tracing::info!("Runtime {} installed successfully", runtime.name);
        Ok(())
    }

    /// Remove a runtime's DLLs from a bottle and clean up registry overrides.
    pub fn uninstall_from_bottle(
        &self,
        runtime_type: RuntimeType,
        bottle_path: &Path,
    ) -> Result<(), RuntimeError> {
        let system32 = bottle_path.join("drive_c/windows/system32");
        let syswow64 = bottle_path.join("drive_c/windows/syswow64");

        tracing::info!(
            "Uninstalling {} from bottle at {}",
            runtime_type,
            bottle_path.display()
        );

        match runtime_type {
            RuntimeType::Dxvk => {
                let dlls = ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"];
                remove_dlls(&system32, &dlls);
                remove_dlls(&syswow64, &dlls);
                Self::remove_dll_overrides(bottle_path, &dlls)?;
            }
            RuntimeType::Dxmt => {
                let dlls = ["d3d10core.dll", "d3d11.dll", "dxgi.dll"];
                remove_dlls(&system32, &dlls);
                Self::remove_dll_overrides(bottle_path, &dlls)?;
            }
            RuntimeType::MoltenVK => {
                let mvk = bottle_path.join("lib/libMoltenVK.dylib");
                if mvk.exists() {
                    fs::remove_file(&mvk)?;
                    tracing::debug!("Removed {}", mvk.display());
                }
            }
            RuntimeType::D3DMetal => {
                let dlls = ["d3d11.dll", "dxgi.dll"];
                remove_dlls(&system32, &dlls);
                let shared = bottle_path.join("lib/libd3dshared.dylib");
                if shared.exists() {
                    fs::remove_file(&shared)?;
                }
                Self::remove_dll_overrides(bottle_path, &dlls)?;
            }
        }

        tracing::info!("{} uninstalled", runtime_type);
        Ok(())
    }

    /// Write (or merge) DLL override entries into the bottle's `user.reg`.
    ///
    /// Each DLL gets an entry under `[Software\\Wine\\DllOverrides]` with the
    /// specified mode (typically `"native"` or `"native,builtin"`).
    pub fn write_dll_overrides(
        bottle_path: &Path,
        dlls: &[&str],
        mode: &str,
    ) -> Result<(), RuntimeError> {
        let user_reg = bottle_path.join("user.reg");
        let section_header = "[Software\\\\Wine\\\\DllOverrides]";

        let mut lines: Vec<String> = if user_reg.exists() {
            let file = fs::File::open(&user_reg)?;
            io::BufReader::new(file).lines().collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        // Find or create the DllOverrides section.
        let section_idx = lines.iter().position(|l| l.trim() == section_header);

        // Build the new entries we want to insert.
        let new_entries: Vec<String> = dlls
            .iter()
            .map(|dll| {
                let name = dll.strip_suffix(".dll").unwrap_or(dll);
                format!("\"{}\"=\"{}\"", name, mode)
            })
            .collect();

        match section_idx {
            Some(idx) => {
                // Determine the range of existing entries in this section so we
                // can replace/merge them.
                let insert_after = idx + 1;

                // Find the end of this section (next section header or EOF).
                let section_end = lines[insert_after..]
                    .iter()
                    .position(|l| l.starts_with('['))
                    .map(|p| p + insert_after)
                    .unwrap_or(lines.len());

                // Remove existing entries for the DLLs we are overriding so we
                // don't get duplicates.
                let dll_names: Vec<&str> = dlls
                    .iter()
                    .map(|d| d.strip_suffix(".dll").unwrap_or(d))
                    .collect();

                let mut i = insert_after;
                while i < section_end {
                    let trimmed = lines[i].trim();
                    let should_remove = dll_names.iter().any(|name| {
                        trimmed.starts_with(&format!("\"{}\"", name))
                    });
                    if should_remove {
                        lines.remove(i);
                        // Don't increment — next element shifted down.
                    } else {
                        i += 1;
                    }
                }

                // Re-find insertion point (section may have shrunk).
                let insert_at = lines[insert_after..]
                    .iter()
                    .position(|l| l.starts_with('['))
                    .map(|p| p + insert_after)
                    .unwrap_or(lines.len());

                for (j, entry) in new_entries.iter().enumerate() {
                    lines.insert(insert_at + j, entry.clone());
                }
            }
            None => {
                // Section doesn't exist; append it at the end.
                lines.push(String::new());
                lines.push(section_header.to_string());
                for entry in &new_entries {
                    lines.push(entry.clone());
                }
            }
        }

        let mut file = fs::File::create(&user_reg)?;
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                write!(file, "\n")?;
            }
            write!(file, "{}", line)?;
        }
        // Trailing newline.
        write!(file, "\n")?;

        tracing::debug!(
            "Wrote DLL overrides for {:?} to {}",
            dlls,
            user_reg.display()
        );
        Ok(())
    }

    /// Check which runtime types have DLLs present in a bottle's `system32/`.
    pub fn list_installed(bottle_path: &Path) -> Vec<RuntimeType> {
        tracing::debug!(bottle_path = %bottle_path.display(), "Checking installed runtimes in bottle");
        let system32 = bottle_path.join("drive_c/windows/system32");
        let lib_dir = bottle_path.join("lib");
        let mut found = Vec::new();

        // DXVK is the only runtime that ships d3d9.dll.
        if system32.join("d3d9.dll").exists() {
            found.push(RuntimeType::Dxvk);
        }

        // If d3d11.dll exists but not d3d9.dll, it could be DXMT or D3DMetal.
        // We distinguish by the presence of libd3dshared.dylib (D3DMetal ships it).
        if system32.join("d3d11.dll").exists() && !system32.join("d3d9.dll").exists() {
            if lib_dir.join("libd3dshared.dylib").exists() {
                found.push(RuntimeType::D3DMetal);
            } else if system32.join("d3d10core.dll").exists() {
                found.push(RuntimeType::Dxmt);
            }
        }

        if lib_dir.join("libMoltenVK.dylib").exists() {
            found.push(RuntimeType::MoltenVK);
        }

        found
    }

    /// Remove DLL override entries for the given DLLs from `user.reg`.
    fn remove_dll_overrides(bottle_path: &Path, dlls: &[&str]) -> Result<(), RuntimeError> {
        let user_reg = bottle_path.join("user.reg");
        if !user_reg.exists() {
            return Ok(());
        }

        let file = fs::File::open(&user_reg)?;
        let lines: Vec<String> = io::BufReader::new(file)
            .lines()
            .collect::<Result<Vec<_>, _>>()?;

        let dll_names: Vec<&str> = dlls
            .iter()
            .map(|d| d.strip_suffix(".dll").unwrap_or(d))
            .collect();

        let filtered: Vec<&String> = lines
            .iter()
            .filter(|line| {
                let trimmed = line.trim();
                !dll_names
                    .iter()
                    .any(|name| trimmed.starts_with(&format!("\"{}\"", name)))
            })
            .collect();

        let mut out = fs::File::create(&user_reg)?;
        for (i, line) in filtered.iter().enumerate() {
            if i > 0 {
                write!(out, "\n")?;
            }
            write!(out, "{}", line)?;
        }
        write!(out, "\n")?;

        Ok(())
    }
}

/// Copy a single DLL from `src_dir` to `dest_dir`, returning an error if the
/// source file does not exist.
fn copy_dll(src_dir: &Path, dest_dir: &Path, dll_name: &str) -> Result<(), RuntimeError> {
    let src = src_dir.join(dll_name);
    if !src.exists() {
        return Err(RuntimeError::MissingDll(format!(
            "{} not found in {}",
            dll_name,
            src_dir.display()
        )));
    }
    let dest = dest_dir.join(dll_name);
    fs::copy(&src, &dest)?;
    tracing::debug!("Copied {} -> {}", src.display(), dest.display());
    Ok(())
}

/// Silently remove DLLs from a directory (best-effort).
fn remove_dlls(dir: &Path, dlls: &[&str]) {
    for dll in dlls {
        let path = dir.join(dll);
        if path.exists() {
            if let Err(e) = fs::remove_file(&path) {
                tracing::warn!("Failed to remove {}: {e}", path.display());
            } else {
                tracing::debug!("Removed {}", path.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a fake runtime distribution directory with placeholder DLL files.
    fn setup_fake_runtime(tmp: &std::path::Path, rt_type: RuntimeType) -> RuntimeVersion {
        let rt_path = tmp.join("runtimes").join("test-runtime");

        match rt_type {
            RuntimeType::Dxvk => {
                let x64 = rt_path.join("x64");
                std::fs::create_dir_all(&x64).unwrap();
                for dll in &["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"] {
                    std::fs::write(x64.join(dll), "fake dll").unwrap();
                }
            }
            RuntimeType::Dxmt => {
                let x64 = rt_path.join("x64");
                std::fs::create_dir_all(&x64).unwrap();
                for dll in &["d3d10core.dll", "d3d11.dll", "dxgi.dll"] {
                    std::fs::write(x64.join(dll), "fake dll").unwrap();
                }
            }
            RuntimeType::MoltenVK => {
                std::fs::create_dir_all(&rt_path).unwrap();
                std::fs::write(rt_path.join("libMoltenVK.dylib"), "fake dylib").unwrap();
            }
            RuntimeType::D3DMetal => {
                std::fs::create_dir_all(&rt_path).unwrap();
                for dll in &["d3d11.dll", "dxgi.dll"] {
                    std::fs::write(rt_path.join(dll), "fake dll").unwrap();
                }
                std::fs::write(rt_path.join("libd3dshared.dylib"), "fake dylib").unwrap();
            }
        }

        RuntimeVersion {
            name: "test-runtime".to_string(),
            runtime_type: rt_type,
            version: "1.0.0".to_string(),
            path: rt_path,
            installed: false,
        }
    }

    /// Create a fake bottle directory structure.
    fn setup_fake_bottle(tmp: &std::path::Path) -> PathBuf {
        let bottle = tmp.join("bottle");
        std::fs::create_dir_all(bottle.join("drive_c/windows/system32")).unwrap();
        bottle
    }

    #[test]
    fn test_install_dxvk_to_bottle() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = setup_fake_bottle(tmp.path());
        let runtime = setup_fake_runtime(tmp.path(), RuntimeType::Dxvk);
        let installer = RuntimeInstaller::new(tmp.path().to_path_buf());

        installer.install_to_bottle(&runtime, &bottle).unwrap();

        let sys32 = bottle.join("drive_c/windows/system32");
        assert!(sys32.join("d3d9.dll").exists());
        assert!(sys32.join("d3d11.dll").exists());
        assert!(sys32.join("dxgi.dll").exists());
        // user.reg should have been created with DLL overrides
        assert!(bottle.join("user.reg").exists());
    }

    #[test]
    fn test_install_moltenvk_to_bottle() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = setup_fake_bottle(tmp.path());
        let runtime = setup_fake_runtime(tmp.path(), RuntimeType::MoltenVK);
        let installer = RuntimeInstaller::new(tmp.path().to_path_buf());

        installer.install_to_bottle(&runtime, &bottle).unwrap();

        assert!(bottle.join("lib/libMoltenVK.dylib").exists());
    }

    #[test]
    fn test_install_d3dmetal_to_bottle() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = setup_fake_bottle(tmp.path());
        let runtime = setup_fake_runtime(tmp.path(), RuntimeType::D3DMetal);
        let installer = RuntimeInstaller::new(tmp.path().to_path_buf());

        installer.install_to_bottle(&runtime, &bottle).unwrap();

        let sys32 = bottle.join("drive_c/windows/system32");
        assert!(sys32.join("d3d11.dll").exists());
        assert!(sys32.join("dxgi.dll").exists());
        assert!(bottle.join("lib/libd3dshared.dylib").exists());
    }

    #[test]
    fn test_install_to_invalid_bottle() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = setup_fake_runtime(tmp.path(), RuntimeType::Dxvk);
        let installer = RuntimeInstaller::new(tmp.path().to_path_buf());

        // Bottle with no system32
        let bad_bottle = tmp.path().join("bad_bottle");
        std::fs::create_dir_all(&bad_bottle).unwrap();

        let result = installer.install_to_bottle(&runtime, &bad_bottle);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_installed_dxvk() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = setup_fake_bottle(tmp.path());
        let sys32 = bottle.join("drive_c/windows/system32");
        std::fs::write(sys32.join("d3d9.dll"), "fake").unwrap();

        let installed = RuntimeInstaller::list_installed(&bottle);
        assert!(installed.contains(&RuntimeType::Dxvk));
    }

    #[test]
    fn test_list_installed_moltenvk() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = setup_fake_bottle(tmp.path());
        let lib = bottle.join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("libMoltenVK.dylib"), "fake").unwrap();

        let installed = RuntimeInstaller::list_installed(&bottle);
        assert!(installed.contains(&RuntimeType::MoltenVK));
    }

    #[test]
    fn test_list_installed_d3dmetal() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = setup_fake_bottle(tmp.path());
        let sys32 = bottle.join("drive_c/windows/system32");
        let lib = bottle.join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(sys32.join("d3d11.dll"), "fake").unwrap();
        std::fs::write(lib.join("libd3dshared.dylib"), "fake").unwrap();

        let installed = RuntimeInstaller::list_installed(&bottle);
        assert!(installed.contains(&RuntimeType::D3DMetal));
    }

    #[test]
    fn test_list_installed_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let bottle = setup_fake_bottle(tmp.path());
        let installed = RuntimeInstaller::list_installed(&bottle);
        assert!(installed.is_empty());
    }
}
