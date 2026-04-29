use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Translation layer backend for DirectX-to-Metal/Vulkan rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsBackend {
    /// Apple's D3DMetal (Game Porting Toolkit)
    D3DMetal,
    /// DXMT — DirectX-to-Metal translation
    DXMT,
    /// DXVK via MoltenVK (Vulkan-to-Metal)
    DxvkMoltenVK,
    /// DXVK via Kosmic Krisp (native Vulkan on macOS)
    DxvkKosmicKrisp,
    /// vkd3d-proton for DirectX 12 via Vulkan
    Vkd3dProton,
    /// Automatic backend selection based on game database
    Auto,
}

impl fmt::Display for GraphicsBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::D3DMetal => write!(f, "D3DMetal"),
            Self::DXMT => write!(f, "DXMT"),
            Self::DxvkMoltenVK => write!(f, "DxvkMoltenVK"),
            Self::DxvkKosmicKrisp => write!(f, "DxvkKosmicKrisp"),
            Self::Vkd3dProton => write!(f, "Vkd3dProton"),
            Self::Auto => write!(f, "Auto"),
        }
    }
}

impl FromStr for GraphicsBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "d3dmetal" => Ok(Self::D3DMetal),
            "dxmt" => Ok(Self::DXMT),
            "dxvkmoltenvk" | "dxvk_moltenvk" => Ok(Self::DxvkMoltenVK),
            "dxvkkosmickrisp" | "dxvk_kosmickrisp" => Ok(Self::DxvkKosmicKrisp),
            "vkd3dproton" | "vkd3d_proton" => Ok(Self::Vkd3dProton),
            "auto" => Ok(Self::Auto),
            _ => Err(format!("unknown graphics backend: {s}")),
        }
    }
}

/// Compatibility rating for a game title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompatStatus {
    Platinum,
    Gold,
    Silver,
    Bronze,
    Borked,
    Unknown,
}

impl fmt::Display for CompatStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platinum => write!(f, "Platinum"),
            Self::Gold => write!(f, "Gold"),
            Self::Silver => write!(f, "Silver"),
            Self::Bronze => write!(f, "Bronze"),
            Self::Borked => write!(f, "Borked"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl FromStr for CompatStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "platinum" => Ok(Self::Platinum),
            "gold" => Ok(Self::Gold),
            "silver" => Ok(Self::Silver),
            "bronze" => Ok(Self::Bronze),
            "borked" => Ok(Self::Borked),
            "unknown" => Ok(Self::Unknown),
            _ => Err(format!("unknown compat status: {s}")),
        }
    }
}

/// A record of a game's compatibility information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRecord {
    pub steam_app_id: Option<u32>,
    pub exe_hash: Option<String>,
    pub title: String,
    pub backend: GraphicsBackend,
    pub compat_status: CompatStatus,
    /// JSON-encoded Wine DLL overrides
    pub wine_overrides: String,
    pub known_issues: String,
    pub last_tested: String,
    pub notes: String,
}

/// A database-layer record for a community compatibility report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatReportRecord {
    pub game_id: String,
    pub reporter_hash: String,
    pub status: String,
    pub backend: String,
    pub fps_avg: Option<f32>,
    pub notes: String,
    pub timestamp: String,
}

/// A commit from the Proton repository being tracked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonCommit {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: String,
    /// JSON-encoded list of affected file paths
    pub affected_files: String,
    pub classification: String,
    pub transferability: String,
    pub applied: bool,
    /// Source: "proton" or "crossover"
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String { "proton".to_string() }

/// Recommended settings for a game, stored in the game_recommended_settings table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRecommendedSettings {
    pub steam_app_id: u32,
    pub msync_enabled: Option<bool>,
    pub esync_enabled: Option<bool>,
    pub rosetta_x87: Option<bool>,
    pub async_shader: Option<bool>,
    pub metalfx_upscaling: Option<bool>,
    pub dxr_ray_tracing: Option<bool>,
    pub fsr_enabled: Option<bool>,
    pub large_address_aware: Option<bool>,
    /// JSON object of DLL overrides: {"dll_name": "mode"}
    pub wine_dll_overrides: String,
    /// JSON object of extra environment variables: {"KEY": "VALUE"}
    pub env_vars: String,
    pub windows_version: Option<String>,
    pub launch_args: Option<String>,
    pub auto_apply_patches: Option<bool>,
    pub cpu_topology: Option<String>,
    pub required_dependencies: String,
    pub registry_entries: String,
    pub exe_override: Option<String>,
    pub audio_latency_ms: Option<i32>,
    pub hidpi_mode: Option<bool>,
}

/// A record from the game_binary_patches table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameBinaryPatchRecord {
    pub id: i64,
    pub steam_app_id: u32,
    pub exe_name: String,
    pub exe_hash: String,
    pub description: String,
    pub search_pattern: Vec<u8>,
    pub replace_pattern: Vec<u8>,
    pub enabled: bool,
    pub patch_mode: String,
    pub file_offset: Option<i64>,
}
