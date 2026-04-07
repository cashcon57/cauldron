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
