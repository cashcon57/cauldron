use std::collections::HashMap;

/// Configuration for Wine synchronization primitives.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Enable MSync (macOS-native synchronization).
    pub msync: bool,
    /// Enable ESync (eventfd-based synchronization).
    pub esync: bool,
    /// MSync queue limit.
    pub msync_qlimit: u32,
}

/// Game-specific fixes and workarounds.
#[derive(Debug, Clone)]
pub struct GameFixes {
    /// Enable AMD FidelityFX Super Resolution.
    pub fsr: bool,
    /// FSR sharpening strength (0 = max sharpness, 5 = least).
    pub fsr_strength: u8,
    /// Hostnames to block via /etc/hosts in the prefix (anti-cheat, telemetry).
    pub block_hosts: Vec<String>,
    /// Enable Large Address Aware flag for 32-bit executables.
    pub large_address_aware: bool,
    /// Disable short filename (8.3) generation.
    pub disable_sfn: bool,
}

/// Build environment variables for the given sync configuration.
pub fn build_sync_env(config: &SyncConfig) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    if config.msync {
        vars.insert("WINEMSYNC".to_string(), "1".to_string());
    }

    if config.esync {
        vars.insert("WINEESYNC".to_string(), "1".to_string());
    }

    if config.msync && config.msync_qlimit > 0 {
        vars.insert(
            "WINEMSYNC_QLIMIT".to_string(),
            config.msync_qlimit.to_string(),
        );
    }

    vars
}

/// Build environment variables for game-specific fixes.
pub fn build_fixes_env(config: &GameFixes) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    if config.fsr {
        vars.insert("WINE_FULLSCREEN_FSR".to_string(), "1".to_string());
        vars.insert(
            "WINE_FULLSCREEN_FSR_STRENGTH".to_string(),
            config.fsr_strength.to_string(),
        );
    }

    if config.large_address_aware {
        vars.insert("WINE_LARGE_ADDRESS_AWARE".to_string(), "1".to_string());
    }

    if config.disable_sfn {
        vars.insert("WINE_DISABLE_SFN".to_string(), "1".to_string());
    }

    // block_hosts is applied at prefix level, not via env vars,
    // but we signal intent via env so the runner can act on it
    if !config.block_hosts.is_empty() {
        vars.insert(
            "CAULDRON_BLOCK_HOSTS".to_string(),
            config.block_hosts.join(","),
        );
    }

    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_sync_env_msync_only() {
        let config = SyncConfig {
            msync: true,
            esync: false,
            msync_qlimit: 0,
        };
        let vars = build_sync_env(&config);
        assert_eq!(vars.get("WINEMSYNC"), Some(&"1".to_string()));
        assert!(!vars.contains_key("WINEESYNC"));
        assert!(!vars.contains_key("WINEMSYNC_QLIMIT"));
    }

    #[test]
    fn test_build_sync_env_esync_only() {
        let config = SyncConfig {
            msync: false,
            esync: true,
            msync_qlimit: 0,
        };
        let vars = build_sync_env(&config);
        assert_eq!(vars.get("WINEESYNC"), Some(&"1".to_string()));
        assert!(!vars.contains_key("WINEMSYNC"));
    }

    #[test]
    fn test_build_sync_env_both_with_qlimit() {
        let config = SyncConfig {
            msync: true,
            esync: true,
            msync_qlimit: 256,
        };
        let vars = build_sync_env(&config);
        assert_eq!(vars.get("WINEMSYNC"), Some(&"1".to_string()));
        assert_eq!(vars.get("WINEESYNC"), Some(&"1".to_string()));
        assert_eq!(vars.get("WINEMSYNC_QLIMIT"), Some(&"256".to_string()));
    }

    #[test]
    fn test_build_sync_env_none() {
        let config = SyncConfig {
            msync: false,
            esync: false,
            msync_qlimit: 0,
        };
        let vars = build_sync_env(&config);
        assert!(vars.is_empty());
    }

    #[test]
    fn test_build_fixes_env_fsr() {
        let config = GameFixes {
            fsr: true,
            fsr_strength: 3,
            block_hosts: vec![],
            large_address_aware: false,
            disable_sfn: false,
        };
        let vars = build_fixes_env(&config);
        assert_eq!(vars.get("WINE_FULLSCREEN_FSR"), Some(&"1".to_string()));
        assert_eq!(vars.get("WINE_FULLSCREEN_FSR_STRENGTH"), Some(&"3".to_string()));
    }

    #[test]
    fn test_build_fixes_env_large_address() {
        let config = GameFixes {
            fsr: false,
            fsr_strength: 0,
            block_hosts: vec![],
            large_address_aware: true,
            disable_sfn: false,
        };
        let vars = build_fixes_env(&config);
        assert_eq!(vars.get("WINE_LARGE_ADDRESS_AWARE"), Some(&"1".to_string()));
    }

    #[test]
    fn test_build_fixes_env_disable_sfn() {
        let config = GameFixes {
            fsr: false,
            fsr_strength: 0,
            block_hosts: vec![],
            large_address_aware: false,
            disable_sfn: true,
        };
        let vars = build_fixes_env(&config);
        assert_eq!(vars.get("WINE_DISABLE_SFN"), Some(&"1".to_string()));
    }

    #[test]
    fn test_build_fixes_env_block_hosts() {
        let config = GameFixes {
            fsr: false,
            fsr_strength: 0,
            block_hosts: vec!["evil.com".to_string(), "tracking.io".to_string()],
            large_address_aware: false,
            disable_sfn: false,
        };
        let vars = build_fixes_env(&config);
        assert_eq!(
            vars.get("CAULDRON_BLOCK_HOSTS"),
            Some(&"evil.com,tracking.io".to_string())
        );
    }

    #[test]
    fn test_build_fixes_env_empty() {
        let config = GameFixes {
            fsr: false,
            fsr_strength: 0,
            block_hosts: vec![],
            large_address_aware: false,
            disable_sfn: false,
        };
        let vars = build_fixes_env(&config);
        assert!(vars.is_empty());
    }
}
