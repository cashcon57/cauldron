use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Configure failed: {0}")]
    ConfigureFailed(String),
    #[error("Build failed: {0}")]
    BuildFailed(String),
    #[error("Source directory not found: {0}")]
    SourceNotFound(PathBuf),
    #[error("Missing dependency: {0}")]
    MissingDependency(String),
    #[error("Install failed: {0}")]
    InstallFailed(String),
}

/// The target architecture for the Wine build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildArch {
    /// 64-bit only (arm64 on Apple Silicon).
    Win64,
    /// 32-bit WoW64 support via Wine's new WoW64 mode.
    Wow64,
}

/// Build configuration for compiling Wine from source.
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// Path to the Wine source tree.
    pub source_dir: PathBuf,
    /// Where to install the built Wine (`make install DESTDIR=...`).
    pub install_dir: PathBuf,
    /// Build directory (out-of-tree build).
    pub build_dir: PathBuf,
    /// Target architecture.
    pub arch: BuildArch,
    /// Number of parallel build jobs (defaults to CPU count).
    pub jobs: usize,
    /// Extra flags to pass to `./configure`.
    pub extra_configure_flags: Vec<String>,
    /// Extra environment variables for the build.
    pub extra_env: HashMap<String, String>,
    /// Whether to enable MSync support in the build.
    pub enable_msync: bool,
    /// Whether to enable the WoW64 (32-on-64) compatibility mode.
    pub enable_wow64: bool,
}

impl BuildConfig {
    /// Create a default build configuration for the given source and output directories.
    pub fn new(source_dir: PathBuf, install_dir: PathBuf) -> Self {
        let build_dir = source_dir.parent()
            .unwrap_or(Path::new("/tmp"))
            .join("wine-build");

        let jobs = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        Self {
            source_dir,
            install_dir,
            build_dir,
            arch: BuildArch::Win64,
            jobs,
            extra_configure_flags: Vec::new(),
            extra_env: HashMap::new(),
            enable_msync: true,
            enable_wow64: false,
        }
    }
}

/// Result of a build operation.
#[derive(Debug, Clone)]
pub struct BuildResult {
    /// Whether the build succeeded.
    pub success: bool,
    /// Path to the installed Wine binary, if build succeeded.
    pub wine_binary: Option<PathBuf>,
    /// Total build duration.
    pub duration: Duration,
    /// Build log (last N lines of output).
    pub log_tail: String,
    /// The configure flags that were used.
    pub configure_flags: Vec<String>,
}

/// Builds Wine from source on macOS.
///
/// This replaces the need for CrossOver's proprietary Wine builds by compiling
/// Wine directly from the Cauldron-patched source tree. The builder handles:
///
/// 1. Dependency checking (homebrew packages needed for macOS Wine builds)
/// 2. Configure with macOS-appropriate flags
/// 3. Parallel make
/// 4. Installation to a local prefix
pub struct WineBuilder {
    pub config: BuildConfig,
}

impl WineBuilder {
    pub fn new(config: BuildConfig) -> Self {
        Self { config }
    }

    /// Check that required build dependencies are available.
    ///
    /// On macOS, Wine builds typically require packages installed via Homebrew:
    /// - mingw-w64 (cross-compiler for PE DLLs)
    /// - bison, flex (parser generators)
    /// - freetype (font rendering)
    /// - molten-vk (optional, for Vulkan support)
    pub fn check_dependencies(&self) -> Result<DependencyReport, BuildError> {
        tracing::info!("Checking build dependencies");

        let mut report = DependencyReport {
            missing: Vec::new(),
            found: Vec::new(),
        };

        let required = [
            ("x86_64-w64-mingw32-gcc", "mingw-w64", true),
            ("bison", "bison", true),
            ("flex", "flex", true),
        ];

        let optional = [
            ("freetype-config", "freetype", false),
            ("pkg-config", "pkg-config", false),
        ];

        for (binary, package, required) in required.iter().chain(optional.iter()) {
            let found = Command::new("which")
                .arg(binary)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if found {
                report.found.push(package.to_string());
            } else if *required {
                report.missing.push(DepInfo {
                    name: package.to_string(),
                    install_cmd: format!("brew install {}", package),
                    required: true,
                });
            } else {
                report.missing.push(DepInfo {
                    name: package.to_string(),
                    install_cmd: format!("brew install {}", package),
                    required: false,
                });
            }
        }

        // Check for Xcode command-line tools (needed for Metal headers)
        let xcode_check = Command::new("xcode-select")
            .arg("-p")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if xcode_check {
            report.found.push("xcode-cli-tools".to_string());
        } else {
            report.missing.push(DepInfo {
                name: "xcode-cli-tools".to_string(),
                install_cmd: "xcode-select --install".to_string(),
                required: true,
            });
        }

        tracing::info!(
            found = report.found.len(),
            missing = report.missing.len(),
            "Dependency check complete"
        );

        Ok(report)
    }

    /// Run `./configure` with macOS-appropriate flags.
    pub fn configure(&self) -> Result<(), BuildError> {
        if !self.config.source_dir.exists() {
            return Err(BuildError::SourceNotFound(self.config.source_dir.clone()));
        }

        std::fs::create_dir_all(&self.config.build_dir)?;

        let configure_path = self.config.source_dir.join("configure");
        if !configure_path.exists() {
            // Some Wine trees need autoreconf first
            tracing::info!("Running autoreconf in source directory");
            let status = Command::new("autoreconf")
                .arg("-fi")
                .current_dir(&self.config.source_dir)
                .status()
                .map_err(|e| BuildError::ConfigureFailed(format!("autoreconf failed: {e}")))?;

            if !status.success() {
                return Err(BuildError::ConfigureFailed(
                    "autoreconf failed".to_string(),
                ));
            }
        }

        let mut args = vec![
            format!("--prefix={}", self.config.install_dir.display()),
        ];

        // Architecture flags
        match self.config.arch {
            BuildArch::Win64 => {
                args.push("--enable-win64".to_string());
            }
            BuildArch::Wow64 => {
                args.push("--enable-win64".to_string());
                if self.config.enable_wow64 {
                    args.push("--enable-archs=i386,x86_64".to_string());
                }
            }
        }

        // macOS-specific flags
        args.push("--with-no-phys-exec".to_string());
        args.push("--without-oss".to_string());
        args.push("--without-v4l2".to_string());
        args.push("--without-alsa".to_string());

        // MSync support (Mach semaphore-based sync for macOS)
        if self.config.enable_msync {
            args.push("--with-msync".to_string());
        }

        // MinGW cross-compiler for PE DLLs (required for modern Wine)
        args.push("--with-mingw".to_string());

        // Extra user-specified flags
        args.extend(self.config.extra_configure_flags.clone());

        tracing::info!(
            flags = ?args,
            build_dir = %self.config.build_dir.display(),
            "Running configure"
        );

        let mut cmd = Command::new(configure_path);
        cmd.args(&args)
            .current_dir(&self.config.build_dir)
            .envs(&self.config.extra_env);

        // Set macOS SDK path if available
        if let Ok(sdk_path) = get_macos_sdk_path() {
            cmd.env("SDKROOT", &sdk_path);
            // Help the cross-compiler find macOS headers
            cmd.env(
                "CFLAGS",
                format!(
                    "-isysroot {} {}",
                    sdk_path,
                    self.config.extra_env.get("CFLAGS").unwrap_or(&String::new())
                ),
            );
        }

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("Configure failed:\n{}", stderr);
            return Err(BuildError::ConfigureFailed(
                last_n_lines(&stderr, 20),
            ));
        }

        tracing::info!("Configure succeeded");
        Ok(())
    }

    /// Run `make` to compile Wine.
    pub fn build(&self) -> Result<BuildResult, BuildError> {
        let start = Instant::now();

        tracing::info!(
            jobs = self.config.jobs,
            build_dir = %self.config.build_dir.display(),
            "Starting Wine build"
        );

        let output = Command::new("make")
            .args(["-j", &self.config.jobs.to_string()])
            .current_dir(&self.config.build_dir)
            .envs(&self.config.extra_env)
            .output()?;

        let duration = start.elapsed();
        let log_tail = last_n_lines(
            &String::from_utf8_lossy(&output.stderr),
            50,
        );

        if !output.status.success() {
            tracing::error!("Build failed after {:?}", duration);
            return Ok(BuildResult {
                success: false,
                wine_binary: None,
                duration,
                log_tail,
                configure_flags: vec![],
            });
        }

        tracing::info!("Build succeeded in {:?}", duration);
        Ok(BuildResult {
            success: true,
            wine_binary: None, // Set after install
            duration,
            log_tail,
            configure_flags: vec![],
        })
    }

    /// Run `make install` to install Wine into the install directory.
    pub fn install(&self) -> Result<BuildResult, BuildError> {
        let start = Instant::now();

        tracing::info!(
            install_dir = %self.config.install_dir.display(),
            "Installing Wine"
        );

        std::fs::create_dir_all(&self.config.install_dir)?;

        let output = Command::new("make")
            .args(["install"])
            .current_dir(&self.config.build_dir)
            .envs(&self.config.extra_env)
            .output()?;

        let duration = start.elapsed();

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BuildError::InstallFailed(last_n_lines(&stderr, 20)));
        }

        // Find the installed wine binary
        let wine_bin = find_built_wine_binary(&self.config.install_dir);

        tracing::info!(
            wine_binary = ?wine_bin,
            "Wine installed in {:?}",
            duration
        );

        Ok(BuildResult {
            success: true,
            wine_binary: wine_bin,
            duration,
            log_tail: String::new(),
            configure_flags: vec![],
        })
    }

    /// Full build pipeline: check deps, configure, build, install.
    pub fn full_build(&self) -> Result<BuildResult, BuildError> {
        // Check dependencies
        let deps = self.check_dependencies()?;
        let critical_missing: Vec<_> = deps.missing.iter().filter(|d| d.required).collect();
        if !critical_missing.is_empty() {
            let names: Vec<_> = critical_missing.iter().map(|d| d.name.as_str()).collect();
            let cmds: Vec<_> = critical_missing.iter().map(|d| d.install_cmd.as_str()).collect();
            return Err(BuildError::MissingDependency(format!(
                "Missing required: {}. Install with:\n{}",
                names.join(", "),
                cmds.join("\n"),
            )));
        }

        // Configure
        self.configure()?;

        // Build
        let build_result = self.build()?;
        if !build_result.success {
            return Err(BuildError::BuildFailed(build_result.log_tail));
        }

        // Install
        self.install()
    }

    /// Clean the build directory.
    pub fn clean(&self) -> Result<(), BuildError> {
        if self.config.build_dir.exists() {
            tracing::info!("Cleaning build directory: {}", self.config.build_dir.display());
            let status = Command::new("make")
                .arg("clean")
                .current_dir(&self.config.build_dir)
                .status();

            // If make clean fails, just remove the directory
            if status.is_err() || !status.unwrap().success() {
                std::fs::remove_dir_all(&self.config.build_dir)?;
            }
        }
        Ok(())
    }
}

/// Report on build dependency availability.
#[derive(Debug, Clone)]
pub struct DependencyReport {
    pub missing: Vec<DepInfo>,
    pub found: Vec<String>,
}

impl DependencyReport {
    pub fn all_required_present(&self) -> bool {
        !self.missing.iter().any(|d| d.required)
    }
}

/// Information about a build dependency.
#[derive(Debug, Clone)]
pub struct DepInfo {
    pub name: String,
    pub install_cmd: String,
    pub required: bool,
}

/// Get the macOS SDK path from xcrun.
fn get_macos_sdk_path() -> Result<String, BuildError> {
    let output = Command::new("xcrun")
        .args(["--show-sdk-path"])
        .output()
        .map_err(|e| BuildError::ConfigureFailed(format!("xcrun failed: {e}")))?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Find the wine/wine64 binary in an installed Wine prefix.
fn find_built_wine_binary(install_dir: &Path) -> Option<PathBuf> {
    let candidates = [
        "bin/wine64",
        "bin/wine",
        "usr/local/bin/wine64",
        "usr/local/bin/wine",
    ];

    for candidate in &candidates {
        let path = install_dir.join(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Return the last N lines of a string.
fn last_n_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_config_defaults() {
        let config = BuildConfig::new(
            PathBuf::from("/tmp/wine-source"),
            PathBuf::from("/tmp/wine-install"),
        );

        assert_eq!(config.source_dir, PathBuf::from("/tmp/wine-source"));
        assert_eq!(config.install_dir, PathBuf::from("/tmp/wine-install"));
        assert!(config.jobs > 0);
        assert!(config.enable_msync);
        assert!(!config.enable_wow64);
        assert_eq!(config.arch, BuildArch::Win64);
    }

    #[test]
    fn test_build_result_construction() {
        let result = BuildResult {
            success: true,
            wine_binary: Some(PathBuf::from("/tmp/wine/bin/wine64")),
            duration: Duration::from_secs(120),
            log_tail: String::new(),
            configure_flags: vec!["--enable-win64".to_string()],
        };

        assert!(result.success);
        assert!(result.wine_binary.is_some());
    }

    #[test]
    fn test_dependency_report() {
        let report = DependencyReport {
            missing: vec![DepInfo {
                name: "optional-dep".to_string(),
                install_cmd: "brew install optional-dep".to_string(),
                required: false,
            }],
            found: vec!["required-dep".to_string()],
        };

        assert!(report.all_required_present());
    }

    #[test]
    fn test_dependency_report_missing_required() {
        let report = DependencyReport {
            missing: vec![DepInfo {
                name: "mingw-w64".to_string(),
                install_cmd: "brew install mingw-w64".to_string(),
                required: true,
            }],
            found: vec![],
        };

        assert!(!report.all_required_present());
    }

    #[test]
    fn test_last_n_lines() {
        let text = "line1\nline2\nline3\nline4\nline5";
        assert_eq!(last_n_lines(text, 2), "line4\nline5");
        assert_eq!(last_n_lines(text, 10), text);
        assert_eq!(last_n_lines("", 5), "");
    }

    #[test]
    fn test_find_built_wine_binary_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(find_built_wine_binary(tmp.path()).is_none());
    }

    #[test]
    fn test_find_built_wine_binary_found() {
        let tmp = tempfile::tempdir().unwrap();
        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join("wine64"), "fake").unwrap();

        let result = find_built_wine_binary(tmp.path());
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("wine64"));
    }

    #[test]
    fn test_source_not_found_error() {
        let config = BuildConfig::new(
            PathBuf::from("/nonexistent/wine-source"),
            PathBuf::from("/tmp/install"),
        );
        let builder = WineBuilder::new(config);
        let result = builder.configure();
        assert!(result.is_err());
    }
}
