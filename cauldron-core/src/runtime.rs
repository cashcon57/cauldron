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
                // DXVK translates D3D9/10/11 to Vulkan.
                // Install as Wine builtins so they work for Steam-launched games.
                let all_dlls = ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"];
                let x64_dir = runtime.path.join("x64");
                let x32_dir = runtime.path.join("x32");

                let home = std::env::var("HOME").unwrap_or_default();
                let wine_pe = PathBuf::from(&home).join("Library/Cauldron/wine/lib/wine/x86_64-windows");

                let mut installed_dlls = Vec::new();
                for dll in &all_dlls {
                    if x64_dir.join(dll).exists() {
                        // Install to Wine builtins (backup original)
                        if wine_pe.exists() {
                            let dest = wine_pe.join(dll);
                            let backup = wine_pe.join(format!("{}.wine-orig", dll));
                            if dest.exists() && !backup.exists() {
                                let _ = fs::copy(&dest, &backup);
                            }
                            fs::copy(&x64_dir.join(dll), &dest)?;
                        }
                        // Also install to system32
                        copy_dll(&x64_dir, &system32, dll)?;
                        installed_dlls.push(*dll);
                    }
                }

                if has_wow64 && x32_dir.exists() {
                    for dll in &all_dlls {
                        if x32_dir.join(dll).exists() {
                            copy_dll(&x32_dir, &syswow64, dll)?;
                        }
                    }
                }

                let dll_refs: Vec<&str> = installed_dlls.iter().copied().collect();
                Self::write_dll_overrides(bottle_path, &dll_refs, "native")?;
            }
            RuntimeType::Dxmt => {
                // DXMT requires both PE DLLs and a Unix .so bridge.
                // Install as Wine builtins (replace in lib/wine/) so they work
                // for ALL processes including games launched via Steam -applaunch.
                let x64_win_dir = if runtime.path.join("x86_64-windows").exists() {
                    runtime.path.join("x86_64-windows")
                } else {
                    runtime.path.join("x64")
                };
                let x64_unix_dir = runtime.path.join("x86_64-unix");

                // Find Wine's install directories
                let home = std::env::var("HOME").unwrap_or_default();
                let wine_pe = PathBuf::from(&home).join("Library/Cauldron/wine/lib/wine/x86_64-windows");
                let wine_unix = PathBuf::from(&home).join("Library/Cauldron/wine/lib/wine/x86_64-unix");

                // PE DLLs: d3d11, d3d10core, dxgi, winemetal
                let pe_dlls = ["d3d11.dll", "d3d10core.dll", "dxgi.dll", "winemetal.dll"];
                for dll in &pe_dlls {
                    let src = x64_win_dir.join(dll);
                    if src.exists() {
                        // Install to Wine's builtin dir (backup original)
                        if wine_pe.exists() {
                            let dest = wine_pe.join(dll);
                            let backup = wine_pe.join(format!("{}.wine-orig", dll));
                            if dest.exists() && !backup.exists() {
                                let _ = fs::copy(&dest, &backup);
                            }
                            fs::copy(&src, &dest)?;
                            tracing::info!("Installed DXMT {} to Wine builtins", dll);
                        }
                        // Also install to system32 for good measure
                        copy_dll(&x64_win_dir, &system32, dll)?;
                    }
                }

                // Unix bridge: winemetal.so (the actual Metal rendering engine)
                let winemetal_so = x64_unix_dir.join("winemetal.so");
                if winemetal_so.exists() && wine_unix.exists() {
                    fs::copy(&winemetal_so, wine_unix.join("winemetal.so"))?;
                    tracing::info!("Installed DXMT winemetal.so to Wine unix dir");
                }

                let dlls = ["d3d10core.dll", "d3d11.dll", "dxgi.dll"];
                Self::write_dll_overrides(bottle_path, &dlls, "native")?;
            }
            RuntimeType::MoltenVK => {
                // MoltenVK layout varies: direct, or MoltenVK/MoltenVK/dylib/macOS/
                let candidates = [
                    runtime.path.join("libMoltenVK.dylib"),
                    runtime.path.join("MoltenVK/MoltenVK/dylib/macOS/libMoltenVK.dylib"),
                    runtime.path.join("MoltenVK/dylib/macOS/libMoltenVK.dylib"),
                    runtime.path.join("dylib/macOS/libMoltenVK.dylib"),
                ];
                let src = candidates.iter().find(|p| p.exists())
                    .ok_or_else(|| RuntimeError::MissingDll(format!(
                        "libMoltenVK.dylib not found in {}",
                        runtime.path.display()
                    )))?;

                let lib_dir = bottle_path.join("lib");
                fs::create_dir_all(&lib_dir)?;
                let dest = lib_dir.join("libMoltenVK.dylib");
                fs::copy(src, &dest)?;
                tracing::debug!("Copied libMoltenVK.dylib -> {}", dest.display());

                // No DLL overrides needed; MoltenVK is a Vulkan ICD, not a
                // Windows DLL.
            }
            RuntimeType::D3DMetal => {
                // D3DMetal requires PE DLLs, Unix .so bridges, and the framework.
                // D3DMetal operates via DLL overrides — .so files dlopen D3DMetal.framework.
                let pe_dlls = ["d3d11.dll", "d3d12.dll", "d3d12core.dll", "dxgi.dll"];
                let unix_sos = ["d3d11.so", "d3d12.so", "dxgi.so"];

                let home = std::env::var("HOME").unwrap_or_default();
                let wine_pe = PathBuf::from(&home).join("Library/Cauldron/wine/lib/wine/x86_64-windows");
                let wine_unix = PathBuf::from(&home).join("Library/Cauldron/wine/lib/wine/x86_64-unix");

                // Look for DLLs in wine/ subdirectory of D3DMetal distribution
                let d3d_wine_dir = runtime.path.parent()
                    .and_then(|p| p.parent())
                    .map(|base| base.join("wine"));

                if let Some(ref wine_dir) = d3d_wine_dir {
                    let pe_dir = wine_dir.join("x86_64-windows");
                    let unix_dir = wine_dir.join("x86_64-unix");

                    // Install PE DLLs as Wine builtins
                    for dll in &pe_dlls {
                        let src = pe_dir.join(dll);
                        if src.exists() {
                            if wine_pe.exists() {
                                let dest = wine_pe.join(dll);
                                let backup = wine_pe.join(format!("{}.wine-orig", dll));
                                if dest.exists() && !backup.exists() {
                                    let _ = fs::copy(&dest, &backup);
                                }
                                let _ = fs::copy(&src, &dest);
                            }
                            copy_dll(&pe_dir, &system32, dll)?;
                        }
                    }

                    // Install Unix .so bridges
                    for so in &unix_sos {
                        let src = unix_dir.join(so);
                        if src.exists() && wine_unix.exists() {
                            let dest = wine_unix.join(so);
                            let backup = wine_unix.join(format!("{}.wine-orig", so));
                            if dest.exists() && !backup.exists() {
                                let _ = fs::copy(&dest, &backup);
                            }
                            fs::copy(&src, &dest)?;
                        }
                    }
                }

                // Copy libd3dshared.dylib if present
                let shared_lib = runtime.path.join("libd3dshared.dylib")
                    .exists()
                    .then(|| runtime.path.join("libd3dshared.dylib"))
                    .or_else(|| d3d_wine_dir.as_ref().map(|d| d.join("libd3dshared.dylib")).filter(|p| p.exists()));
                if let Some(shared) = shared_lib {
                    if wine_unix.exists() {
                        fs::copy(&shared, wine_unix.join("libd3dshared.dylib"))?;
                    }
                    let lib_dir = bottle_path.join("lib");
                    fs::create_dir_all(&lib_dir)?;
                    fs::copy(&shared, lib_dir.join("libd3dshared.dylib"))?;
                }

                let dlls = ["d3d11.dll", "dxgi.dll"];
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
                restore_wine_builtins(&dlls);
            }
            RuntimeType::Dxmt => {
                let dlls = ["d3d10core.dll", "d3d11.dll", "dxgi.dll"];
                remove_dlls(&system32, &dlls);
                Self::remove_dll_overrides(bottle_path, &dlls)?;
                // Restore Wine's original builtins
                restore_wine_builtins(&["d3d11.dll", "d3d10core.dll", "dxgi.dll", "winemetal.dll"]);
                restore_wine_unix_builtins(&["winemetal.so"]);
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
                restore_wine_builtins(&["d3d11.dll", "d3d12.dll", "d3d12core.dll", "dxgi.dll"]);
                restore_wine_unix_builtins(&["d3d11.so", "d3d12.so", "dxgi.so"]);
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

    /// Switch a bottle's graphics backend by uninstalling the old DLLs and
    /// installing the new ones. Handles registry overrides automatically.
    ///
    /// `new_backend` is a `GraphicsBackend` enum value. If `Auto`, restores
    /// Wine's built-in DLLs (removes all overrides).
    pub fn switch_backend(
        &self,
        bottle_path: &Path,
        new_backend: cauldron_db::GraphicsBackend,
    ) -> Result<String, RuntimeError> {
        use cauldron_db::GraphicsBackend;

        // First, uninstall whatever is currently in the bottle
        let current = Self::list_installed(bottle_path);
        for rt in &current {
            if let Err(e) = self.uninstall_from_bottle(*rt, bottle_path) {
                tracing::warn!("Failed to uninstall {}: {}", rt, e);
            }
        }

        // Map GraphicsBackend to RuntimeType + find the runtime on disk
        let target_type = match new_backend {
            GraphicsBackend::DXMT => Some(RuntimeType::Dxmt),
            GraphicsBackend::DxvkMoltenVK | GraphicsBackend::DxvkKosmicKrisp => Some(RuntimeType::Dxvk),
            GraphicsBackend::D3DMetal => Some(RuntimeType::D3DMetal),
            GraphicsBackend::Vkd3dProton => None, // VKD3D is separate
            GraphicsBackend::Auto => None, // Use Wine builtins
        };

        if let Some(rt_type) = target_type {
            // Find the runtime on disk, or auto-download it
            let runtime = match self.find_runtime(rt_type) {
                Some(rt) => rt,
                None => {
                    // Auto-download the runtime
                    tracing::info!("Runtime {} not found locally, downloading...", rt_type);
                    let downloader = crate::runtime_downloader::RuntimeDownloader::new(
                        self.runtimes_dir.parent().unwrap_or(&self.runtimes_dir).to_path_buf()
                    );
                    let component = match rt_type {
                        RuntimeType::Dxvk => crate::runtime_downloader::RuntimeComponent::Dxvk,
                        RuntimeType::Dxmt => crate::runtime_downloader::RuntimeComponent::Dxmt,
                        RuntimeType::MoltenVK => crate::runtime_downloader::RuntimeComponent::MoltenVK,
                        RuntimeType::D3DMetal => crate::runtime_downloader::RuntimeComponent::D3DMetal,
                    };
                    // Find the latest version for this component
                    let releases = downloader.available_releases();
                    let release = releases.iter().find(|r| r.component == component);
                    if let Some(rel) = release {
                        match downloader.download(component, &rel.version) {
                            Ok(_path) => {
                                tracing::info!("Downloaded {} {}", component, rel.version);
                            }
                            Err(e) => {
                                return Err(RuntimeError::NotFound(format!(
                                    "Failed to download {}: {}", rt_type, e
                                )));
                            }
                        }
                    } else {
                        return Err(RuntimeError::NotFound(format!(
                            "No download URL configured for {}", rt_type
                        )));
                    }

                    // Now it should be on disk
                    self.find_runtime(rt_type).ok_or_else(|| {
                        RuntimeError::NotFound(format!(
                            "{} downloaded but not found on disk", rt_type
                        ))
                    })?
                }
            };

            self.install_to_bottle(&runtime, bottle_path)?;

            // For DXVK, also download+install MoltenVK if not present
            if rt_type == RuntimeType::Dxvk {
                if self.find_runtime(RuntimeType::MoltenVK).is_none() {
                    tracing::info!("Auto-downloading MoltenVK for DXVK...");
                    let downloader = crate::runtime_downloader::RuntimeDownloader::new(
                        self.runtimes_dir.parent().unwrap_or(&self.runtimes_dir).to_path_buf()
                    );
                    let releases = downloader.available_releases();
                    if let Some(rel) = releases.iter().find(|r| r.component == crate::runtime_downloader::RuntimeComponent::MoltenVK) {
                        let _ = downloader.download(crate::runtime_downloader::RuntimeComponent::MoltenVK, &rel.version);
                    }
                }
                if let Some(mvk) = self.find_runtime(RuntimeType::MoltenVK) {
                    let _ = self.install_to_bottle(&mvk, bottle_path);
                }
            }

            return Ok(format!("{} installed", rt_type));
        }

        // Auto or no matching runtime — clean state, Wine builtins
        Ok("Using Wine built-in graphics (WineD3D)".to_string())
    }

    /// Find the best available runtime of a given type.
    /// Checks bundled runtimes in deps/runtimes/ first, then downloaded runtimes.
    fn find_runtime(&self, rt_type: RuntimeType) -> Option<RuntimeVersion> {
        let type_dir = match rt_type {
            RuntimeType::Dxvk => "dxvk",
            RuntimeType::Dxmt => "dxmt",
            RuntimeType::MoltenVK => "moltenvk",
            RuntimeType::D3DMetal => "d3dmetal",
        };

        // 1. Check bundled runtimes (shipped with the app)
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        let mut bundled_paths = vec![
            // Relative to working dir (dev builds)
            PathBuf::from("deps/runtimes").join(type_dir),
            // Relative to base_dir (App Support)
            self.runtimes_dir.parent().unwrap_or(&self.runtimes_dir)
                .join("deps/runtimes").join(type_dir),
            // Hardcoded project path (dev builds)
            PathBuf::from("/Users/cashconway/cauldron/deps/runtimes").join(type_dir),
        ];
        // Relative to the executable (release app bundle)
        if let Some(ref exe) = exe_dir {
            bundled_paths.push(exe.join("../Resources/runtimes").join(type_dir));
            bundled_paths.push(exe.join("../../deps/runtimes").join(type_dir));
        }

        for bundled in &bundled_paths {
            if bundled.exists() {
                // Bundled runtimes have DLLs directly (e.g., deps/runtimes/dxvk/x64/)
                // or in a versioned subdir
                if bundled.join("x64").exists() || bundled.join("x86_64-windows").exists() {
                    return Some(RuntimeVersion {
                        name: format!("{}-bundled", type_dir),
                        runtime_type: rt_type,
                        version: "bundled".to_string(),
                        path: bundled.clone(),
                        installed: false,
                    });
                }
                // Check for versioned subdirs
                if let Ok(entries) = fs::read_dir(bundled) {
                    let mut versions: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .collect();
                    versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
                    if let Some(entry) = versions.first() {
                        return Some(RuntimeVersion {
                            name: format!("{}-{}", type_dir, entry.file_name().to_string_lossy()),
                            runtime_type: rt_type,
                            version: entry.file_name().to_string_lossy().to_string(),
                            path: entry.path(),
                            installed: false,
                        });
                    }
                }
            }
        }

        // 2. Check downloaded runtimes
        let dir = self.runtimes_dir.join(type_dir);
        if dir.exists() {
            let mut versions: Vec<_> = fs::read_dir(&dir)
                .ok()?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .collect();
            versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
            if let Some(entry) = versions.first() {
                return Some(RuntimeVersion {
                    name: format!("{}-{}", type_dir, entry.file_name().to_string_lossy()),
                    runtime_type: rt_type,
                    version: entry.file_name().to_string_lossy().to_string(),
                    path: entry.path(),
                    installed: false,
                });
            }
        }

        None
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

/// Restore Wine's original PE builtins from `.wine-orig` backups.
fn restore_wine_builtins(dlls: &[&str]) {
    let home = std::env::var("HOME").unwrap_or_default();
    let wine_pe = PathBuf::from(&home).join("Library/Cauldron/wine/lib/wine/x86_64-windows");
    for dll in dlls {
        let backup = wine_pe.join(format!("{}.wine-orig", dll));
        let target = wine_pe.join(dll);
        if backup.exists() {
            if let Err(e) = fs::copy(&backup, &target) {
                tracing::warn!("Failed to restore {}: {e}", dll);
            } else {
                let _ = fs::remove_file(&backup);
                tracing::info!("Restored Wine builtin {}", dll);
            }
        }
    }
}

/// Restore Wine's original Unix .so builtins from `.wine-orig` backups.
fn restore_wine_unix_builtins(sos: &[&str]) {
    let home = std::env::var("HOME").unwrap_or_default();
    let wine_unix = PathBuf::from(&home).join("Library/Cauldron/wine/lib/wine/x86_64-unix");
    for so in sos {
        let backup = wine_unix.join(format!("{}.wine-orig", so));
        let target = wine_unix.join(so);
        if backup.exists() {
            if let Err(e) = fs::copy(&backup, &target) {
                tracing::warn!("Failed to restore {}: {e}", so);
            } else {
                let _ = fs::remove_file(&backup);
                tracing::info!("Restored Wine unix builtin {}", so);
            }
        } else if target.exists() {
            // No backup means this .so was added by us, not a replacement — remove it
            let _ = fs::remove_file(&target);
            tracing::info!("Removed added {}", so);
        }
    }
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
