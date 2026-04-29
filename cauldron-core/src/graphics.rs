use cauldron_db::GraphicsBackend;
use std::collections::HashMap;
use std::path::PathBuf;

/// Find the bundled runtime DLL directory for WINEDLLPATH.
fn find_bundled_runtime_path(runtime: &str) -> Option<String> {
    let candidates = [
        PathBuf::from("/Users/cashconway/cauldron/deps/runtimes").join(runtime),
        PathBuf::from("deps/runtimes").join(runtime),
    ];
    for dir in &candidates {
        // DXMT uses x86_64-windows/, DXVK uses x64/
        let x64_win = dir.join("x86_64-windows");
        if x64_win.exists() { return Some(x64_win.to_string_lossy().to_string()); }
        let x64 = dir.join("x64");
        if x64.exists() { return Some(x64.to_string_lossy().to_string()); }
        if dir.exists() { return Some(dir.to_string_lossy().to_string()); }
    }
    None
}

/// Configuration for the graphics translation layer.
#[derive(Debug, Clone)]
pub struct GraphicsConfig {
    /// Which backend to use for DirectX translation.
    pub backend: GraphicsBackend,
    /// Enable DXVK async shader compilation.
    pub dxvk_async: bool,
    /// Enable MetalFX spatial upscaling on the swapchain (DXMT).
    pub metalfx_spatial: bool,
    /// MetalFX spatial upscale factor (1.0-2.0, default 2.0).
    pub metalfx_upscale_factor: f32,
    /// Enable DLSS-to-MetalFX translation (D3DMetal/GPTK 3.0).
    pub dlss_metalfx: bool,
    /// Show the Metal performance HUD overlay.
    pub metal_hud: bool,
    /// Enable DirectX Raytracing (DXR) support in D3DMetal (M3+ only).
    pub dxr_enabled: bool,
    /// Enable MoltenVK Metal argument buffers for better performance.
    pub mvk_argument_buffers: bool,
}

/// Build the environment variables needed for the selected graphics configuration.
pub fn build_env_vars(config: &GraphicsConfig) -> HashMap<String, String> {
    tracing::debug!(backend = ?config.backend, dxvk_async = config.dxvk_async, metal_hud = config.metal_hud, "Building graphics environment variables");
    let mut vars = HashMap::new();

    match config.backend {
        GraphicsBackend::D3DMetal => {
            vars.insert("WINED3DMETAL".to_string(), "1".to_string());
            if config.dxr_enabled {
                vars.insert("D3DM_SUPPORT_DXR".to_string(), "1".to_string());
            }
            if config.dlss_metalfx {
                vars.insert("D3DM_ENABLE_METALFX".to_string(), "1".to_string());
            }
        }
        GraphicsBackend::DXMT => {
            if config.metalfx_spatial {
                vars.insert(
                    "DXMT_METALFX_SPATIAL_SWAPCHAIN".to_string(),
                    "1".to_string(),
                );
                // Configurable upscale factor (1.0 = no upscale, 2.0 = double res)
                if config.metalfx_upscale_factor > 0.0 && config.metalfx_upscale_factor != 2.0 {
                    vars.insert(
                        "d3d11.metalSpatialUpscaleFactor".to_string(),
                        format!("{:.1}", config.metalfx_upscale_factor),
                    );
                }
            }
        }
        GraphicsBackend::DxvkMoltenVK | GraphicsBackend::DxvkKosmicKrisp => {
            if config.dxvk_async {
                vars.insert("DXVK_ASYNC".to_string(), "1".to_string());
            }
            if config.mvk_argument_buffers {
                vars.insert(
                    "MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS".to_string(),
                    "1".to_string(),
                );
            }
        }
        GraphicsBackend::Vkd3dProton => {
            if config.mvk_argument_buffers {
                vars.insert(
                    "MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS".to_string(),
                    "1".to_string(),
                );
            }
        }
        GraphicsBackend::Auto => {
            // Auto mode: set common safe defaults
            if config.dxvk_async {
                vars.insert("DXVK_ASYNC".to_string(), "1".to_string());
            }
        }
    }

    if config.metal_hud {
        vars.insert("MTL_HUD_ENABLED".to_string(), "1".to_string());
    }

    // Set WINEDLLOVERRIDES to use native DLLs from DXMT/DXVK and WINEDLLPATH
    // to point Wine at our bundled runtime DLLs.
    // NOTE: WINEDLLOVERRIDES applies to ALL processes in the Wine session.
    // To prevent steamwebhelper.exe from crashing with native d3d11, the launcher
    // writes per-app registry overrides forcing steamwebhelper back to builtin
    // (AppDefaults overrides take precedence over the env var in Wine).
    let (dll_overrides, dll_path) = match config.backend {
        GraphicsBackend::DXMT => {
            let dxmt_path = find_bundled_runtime_path("dxmt");
            (Some("d3d11=n,b;d3d10core=n,b;dxgi=n,b"), dxmt_path)
        }
        GraphicsBackend::DxvkMoltenVK | GraphicsBackend::DxvkKosmicKrisp => {
            let dxvk_path = find_bundled_runtime_path("dxvk");
            (Some("d3d10core=n,b;d3d11=n,b"), dxvk_path)
        }
        GraphicsBackend::Vkd3dProton => {
            (Some("d3d12=n,b"), None)
        }
        GraphicsBackend::D3DMetal => {
            let dxmt_path = find_bundled_runtime_path("dxmt");
            (Some("d3d11=n,b;d3d10core=n,b;dxgi=n,b"), dxmt_path)
        }
        GraphicsBackend::Auto => {
            let dxmt_path = find_bundled_runtime_path("dxmt");
            if dxmt_path.is_some() {
                (Some("d3d11=n,b;d3d10core=n,b;dxgi=n,b"), dxmt_path)
            } else {
                (None, None)
            }
        }
    };
    // Always disable winemenubuilder to prevent Wine dock/menu icon spam.
    // Append any graphics DLL overrides.
    let menu_disable = "winemenubuilder.exe=d";
    if let Some(overrides) = dll_overrides {
        vars.insert("WINEDLLOVERRIDES".to_string(), format!("{};{}", overrides, menu_disable));
    } else {
        vars.insert("WINEDLLOVERRIDES".to_string(), menu_disable.to_string());
    }
    if let Some(path) = dll_path {
        vars.insert("WINEDLLPATH".to_string(), path);
    }

    vars
}

/// Return the DLL names that need native overrides for the given backend.
/// Used by the launcher to protect steamwebhelper.exe by writing per-app
/// registry entries that force these DLLs back to builtin for Steam processes.
pub fn dll_overrides_for_backend(backend: &GraphicsBackend) -> Vec<&'static str> {
    match backend {
        GraphicsBackend::DXMT | GraphicsBackend::D3DMetal => {
            vec!["d3d11", "d3d10core", "dxgi"]
        }
        GraphicsBackend::DxvkMoltenVK | GraphicsBackend::DxvkKosmicKrisp => {
            vec!["d3d10core", "d3d11"]
        }
        GraphicsBackend::Vkd3dProton => {
            vec!["d3d12"]
        }
        GraphicsBackend::Auto => {
            if find_bundled_runtime_path("dxmt").is_some() {
                vec!["d3d11", "d3d10core", "dxgi"]
            } else {
                vec![]
            }
        }
    }
}

/// Automatically select the best graphics backend for a given DirectX version.
///
/// * `dx_version` — the major DirectX version the game uses (9, 10, 11, or 12).
/// * `has_kosmic_krisp` — whether the Kosmic Krisp Vulkan driver is available on
///   this system, enabling `DxvkKosmicKrisp` as an option.
///
/// The selection logic:
/// - DX12 always goes through D3DMetal (Apple's GPTK is the only DX12 path).
/// - DX11/10 prefer DXMT (Metal-native), falling back to DXVK+MoltenVK.
/// - DX9 can only be handled by DXVK (DXMT has no DX9 support).
/// - When Kosmic Krisp is available and the game is DX9/10/11, it can be used
///   as an alternative to MoltenVK under DXVK.
pub fn auto_select_backend(dx_version: u8, has_kosmic_krisp: bool) -> GraphicsBackend {
    tracing::debug!(dx_version = dx_version, has_kosmic_krisp = has_kosmic_krisp, "Auto-selecting graphics backend");
    let result = match dx_version {
        12 => GraphicsBackend::D3DMetal,
        11 | 10 => {
            // DXMT is preferred for DX10/11 on macOS because it translates
            // directly to Metal without the Vulkan intermediary.
            // Fall back to DXVK paths if the caller wants Vulkan-based
            // translation instead.
            GraphicsBackend::DXMT
        }
        9 => {
            // DXMT does not support DX9; DXVK is the only option.
            if has_kosmic_krisp {
                GraphicsBackend::DxvkKosmicKrisp
            } else {
                GraphicsBackend::DxvkMoltenVK
            }
        }
        _ => {
            // Unknown DX version — use Auto and let the runtime figure it out.
            tracing::warn!(
                dx_version = dx_version,
                "Unknown DirectX version, falling back to Auto"
            );
            GraphicsBackend::Auto
        }
    };
    tracing::info!(dx_version = dx_version, backend = ?result, "Auto-selected graphics backend");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config(backend: GraphicsBackend) -> GraphicsConfig {
        GraphicsConfig {
            backend,
            dxvk_async: true,
            metalfx_spatial: true,
            metalfx_upscale_factor: 2.0,
            dlss_metalfx: true,
            metal_hud: false,
            dxr_enabled: true,
            mvk_argument_buffers: true,
        }
    }

    #[test]
    fn test_build_env_vars_d3dmetal() {
        let config = default_config(GraphicsBackend::D3DMetal);
        let vars = build_env_vars(&config);
        assert_eq!(vars.get("D3DM_SUPPORT_DXR"), Some(&"1".to_string()));
        assert!(!vars.contains_key("DXVK_ASYNC"));
    }

    #[test]
    fn test_build_env_vars_dxmt() {
        let config = default_config(GraphicsBackend::DXMT);
        let vars = build_env_vars(&config);
        assert_eq!(vars.get("DXMT_METALFX_SPATIAL_SWAPCHAIN"), Some(&"1".to_string()));
    }

    #[test]
    fn test_build_env_vars_dxvk_moltenvk() {
        let config = default_config(GraphicsBackend::DxvkMoltenVK);
        let vars = build_env_vars(&config);
        assert_eq!(vars.get("DXVK_ASYNC"), Some(&"1".to_string()));
        assert_eq!(vars.get("MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS"), Some(&"1".to_string()));
    }

    #[test]
    fn test_build_env_vars_vkd3d_proton() {
        let config = default_config(GraphicsBackend::Vkd3dProton);
        let vars = build_env_vars(&config);
        assert_eq!(vars.get("MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS"), Some(&"1".to_string()));
        assert!(!vars.contains_key("DXVK_ASYNC"));
    }

    #[test]
    fn test_build_env_vars_auto() {
        let config = default_config(GraphicsBackend::Auto);
        let vars = build_env_vars(&config);
        assert_eq!(vars.get("DXVK_ASYNC"), Some(&"1".to_string()));
    }

    #[test]
    fn test_build_env_vars_metal_hud() {
        let mut config = default_config(GraphicsBackend::Auto);
        config.metal_hud = true;
        let vars = build_env_vars(&config);
        assert_eq!(vars.get("MTL_HUD_ENABLED"), Some(&"1".to_string()));
    }

    #[test]
    fn test_build_env_vars_metal_hud_off() {
        let config = default_config(GraphicsBackend::Auto);
        let vars = build_env_vars(&config);
        assert!(!vars.contains_key("MTL_HUD_ENABLED"));
    }

    #[test]
    fn test_auto_select_dx12() {
        assert_eq!(auto_select_backend(12, false), GraphicsBackend::D3DMetal);
        assert_eq!(auto_select_backend(12, true), GraphicsBackend::D3DMetal);
    }

    #[test]
    fn test_auto_select_dx11() {
        assert_eq!(auto_select_backend(11, false), GraphicsBackend::DXMT);
        assert_eq!(auto_select_backend(11, true), GraphicsBackend::DXMT);
    }

    #[test]
    fn test_auto_select_dx10() {
        assert_eq!(auto_select_backend(10, false), GraphicsBackend::DXMT);
    }

    #[test]
    fn test_auto_select_dx9_no_kosmic() {
        assert_eq!(auto_select_backend(9, false), GraphicsBackend::DxvkMoltenVK);
    }

    #[test]
    fn test_auto_select_dx9_with_kosmic() {
        assert_eq!(auto_select_backend(9, true), GraphicsBackend::DxvkKosmicKrisp);
    }

    #[test]
    fn test_auto_select_unknown_dx() {
        assert_eq!(auto_select_backend(7, false), GraphicsBackend::Auto);
        assert_eq!(auto_select_backend(0, true), GraphicsBackend::Auto);
    }
}
