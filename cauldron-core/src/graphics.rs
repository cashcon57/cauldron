use cauldron_db::GraphicsBackend;
use std::collections::HashMap;

/// Configuration for the graphics translation layer.
#[derive(Debug, Clone)]
pub struct GraphicsConfig {
    /// Which backend to use for DirectX translation.
    pub backend: GraphicsBackend,
    /// Enable DXVK async shader compilation.
    pub dxvk_async: bool,
    /// Enable MetalFX spatial upscaling on the swapchain.
    pub metalfx_spatial: bool,
    /// Show the Metal performance HUD overlay.
    pub metal_hud: bool,
    /// Enable DirectX Raytracing (DXR) support in D3DMetal.
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
            if config.dxr_enabled {
                vars.insert("D3DM_SUPPORT_DXR".to_string(), "1".to_string());
            }
        }
        GraphicsBackend::DXMT => {
            if config.metalfx_spatial {
                vars.insert(
                    "DXMT_METALFX_SPATIAL_SWAPCHAIN".to_string(),
                    "1".to_string(),
                );
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

    vars
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
