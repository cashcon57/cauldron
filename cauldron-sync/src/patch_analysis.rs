//! Automated patch analysis: dry-run checks, impact scoring, affected games detection.

use std::path::Path;
use std::process::Command;

/// Result of analyzing a single patch.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PatchAnalysis {
    pub hash: String,
    /// Whether `git apply --check` succeeds on this patch.
    pub applies_cleanly: Option<bool>,
    /// Conflicting files if dry-run fails.
    pub conflict_files: Vec<String>,
    /// DLLs/components touched by this patch (extracted from affected_files).
    pub affected_dlls: Vec<String>,
    /// Impact score: "low", "medium", "high"
    pub impact: String,
    /// Human-readable impact reason.
    pub impact_reason: String,
    /// Lines added/removed.
    pub lines_added: usize,
    pub lines_removed: usize,
    /// Game titles that may be affected (matched via DLL imports).
    pub affected_games: Vec<String>,
    /// ProtonDB rating for GameConfig patches (if app_id found).
    pub protondb_rating: Option<String>,
    /// Whether auto-adaptation would modify this patch.
    pub can_auto_adapt: bool,
    /// Number of Linux→macOS transforms that would be applied.
    pub adaptation_transform_count: usize,
    /// Adaptation confidence level.
    pub adaptation_confidence: String,
    /// Warnings from the auto-adapter (patterns needing manual review).
    pub adaptation_warnings: Vec<String>,
    /// Modding/gaming impact notes.
    pub modding_impact: Vec<String>,
    /// Suggested action from the classifier.
    pub suggested_action: String,
}

/// Run a dry-run `git apply --check` for a patch diff against a wine source tree.
pub fn dry_run_check(wine_source_dir: &Path, diff: &str) -> (bool, Vec<String>) {
    if diff.is_empty() || !wine_source_dir.join(".git").exists() {
        return (false, vec!["Wine source not initialized".to_string()]);
    }

    let tmp = match tempfile::NamedTempFile::new() {
        Ok(t) => t,
        Err(_) => return (false, vec!["Failed to create temp file".to_string()]),
    };

    if std::fs::write(tmp.path(), diff).is_err() {
        return (false, vec!["Failed to write diff".to_string()]);
    }

    let output = Command::new("git")
        .args(["apply", "--check", "--verbose", tmp.path().to_str().unwrap_or("")])
        .current_dir(wine_source_dir)
        .output();

    match output {
        Ok(out) if out.status.success() => (true, vec![]),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let mut conflicts = Vec::new();
            for line in stderr.lines() {
                let trimmed = line.trim();
                if trimmed.contains("CONFLICT") {
                    conflicts.push(trimmed.to_string());
                } else if let Some(rest) = trimmed.strip_prefix("error: ") {
                    // Extract file path from common git apply errors:
                    // "error: patch failed: dlls/ntdll/sync.c:123"
                    // "error: dlls/ntdll/sync.c: does not exist in index"
                    if let Some(path) = rest.strip_prefix("patch failed: ") {
                        let file = path.split(':').next().unwrap_or(path);
                        if !conflicts.contains(&file.to_string()) {
                            conflicts.push(file.to_string());
                        }
                    } else if rest.contains(": ") {
                        let file = rest.split(": ").next().unwrap_or(rest);
                        // Only include if it looks like a file path
                        if file.contains('/') || file.contains('.') {
                            if !conflicts.contains(&file.to_string()) {
                                conflicts.push(file.to_string());
                            }
                        } else {
                            conflicts.push(trimmed.to_string());
                        }
                    } else {
                        conflicts.push(trimmed.to_string());
                    }
                }
            }
            if conflicts.is_empty() && !stderr.is_empty() {
                conflicts.push("Patch does not apply cleanly".to_string());
            }
            (false, conflicts)
        }
        Err(e) => (false, vec![format!("git error: {e}")]),
    }
}

/// Extract DLLs/Wine components from affected file paths.
pub fn extract_affected_dlls(affected_files: &[String]) -> Vec<String> {
    let mut dlls = Vec::new();
    for file in affected_files {
        // "dlls/d3d11/device.c" → "d3d11"
        if let Some(rest) = file.strip_prefix("dlls/") {
            if let Some(dll_name) = rest.split('/').next() {
                if !dlls.contains(&dll_name.to_string()) {
                    dlls.push(dll_name.to_string());
                }
            }
        }
        // "server/thread.c" → "wine-server"
        if file.starts_with("server/") && !dlls.contains(&"wine-server".to_string()) {
            dlls.push("wine-server".to_string());
        }
        // "loader/" → "wine-loader"
        if file.starts_with("loader/") && !dlls.contains(&"wine-loader".to_string()) {
            dlls.push("wine-loader".to_string());
        }
    }
    dlls
}

/// Compute impact score based on diff characteristics.
pub fn compute_impact(
    diff: &str,
    affected_files: &[String],
    classification: &str,
) -> (String, String, usize, usize) {
    let lines_added = diff.lines().filter(|l| l.starts_with('+') && !l.starts_with("+++")).count();
    let lines_removed = diff.lines().filter(|l| l.starts_with('-') && !l.starts_with("---")).count();
    let total_changed = lines_added + lines_removed;
    let file_count = affected_files.len();

    // If no actual code changes, can't assess risk from diff content
    if total_changed == 0 {
        let reason = if diff.is_empty() {
            "No diff available — patch may need re-sync".to_string()
        } else {
            "No code changes detected in diff".to_string()
        };
        return ("low risk".to_string(), reason, 0, 0);
    }

    // Hot path keywords — only check actual changed lines, not headers/metadata
    let hot_keywords = [
        "mutex", "lock", "semaphore", "sync", "thread", "memory", "alloc",
        "heap", "mmap", "virtual", "ntdll", "kernel32", "critical_section",
    ];
    let changed_lines_text: String = diff
        .lines()
        .filter(|l| (l.starts_with('+') && !l.starts_with("+++")) || (l.starts_with('-') && !l.starts_with("---")))
        .map(|l| l.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    let touches_hot_path = hot_keywords.iter().any(|k| changed_lines_text.contains(k));

    let (impact, reason) = if classification == "BuildSystem" {
        ("low risk".to_string(), "Build/infra change, no runtime impact".to_string())
    } else if total_changed > 500 || file_count > 10 {
        ("high risk".to_string(), format!("{total_changed} lines across {file_count} files"))
    } else if touches_hot_path {
        ("high risk".to_string(), format!("Touches synchronization/memory code ({total_changed} lines)"))
    } else if total_changed > 100 || file_count > 5 {
        ("medium risk".to_string(), format!("{total_changed} lines across {file_count} files"))
    } else if classification == "KernelWorkaround" {
        ("medium risk".to_string(), "Kernel workaround requires macOS adaptation".to_string())
    } else {
        ("low risk".to_string(), format!("{total_changed} lines, {file_count} file(s)"))
    };

    (impact, reason, lines_added, lines_removed)
}

/// Match affected DLLs against game PE imports to find affected games.
/// `game_imports` is a list of (game_title, vec_of_dll_names).
pub fn match_affected_games(
    affected_dlls: &[String],
    game_imports: &[(String, Vec<String>)],
) -> Vec<String> {
    let mut matched = Vec::new();
    for (title, imports) in game_imports {
        for dll in affected_dlls {
            // Match dll name: "d3d11" matches import "d3d11.dll"
            let dll_with_ext = format!("{dll}.dll");
            if imports.iter().any(|imp| imp.eq_ignore_ascii_case(&dll_with_ext) || imp.eq_ignore_ascii_case(dll)) {
                if !matched.contains(title) {
                    matched.push(title.clone());
                }
                break;
            }
        }
    }
    matched
}

/// Detect potential modding/gaming impact from a patch.
pub fn detect_modding_impact(
    affected_dlls: &[String],
    message: &str,
    classification: &str,
) -> Vec<String> {
    let mut impacts = Vec::new();
    let msg_lower = message.to_ascii_lowercase();

    // DLLs that mod loaders commonly hook into
    let mod_loader_dlls = [
        ("ntdll", "SKSE/F4SE/OBSE inject via ntdll hooks — changes may break script extenders"),
        ("kernel32", "Mod loaders use kernel32 for DLL injection — changes may affect mod loading"),
        ("d3d11", "ENB/ReShade hook d3d11 — changes may affect graphics mods"),
        ("d3d9", "ENB/ReShade hook d3d9 — changes may affect graphics mods"),
        ("d3d12", "ReShade hooks d3d12 — changes may affect graphics mods"),
        ("dxgi", "ENB/ReShade/DXVK hook dxgi — changes may affect graphics mod chains"),
        ("dinput", "Some mods hook dinput for custom controls — changes may affect input mods"),
        ("dinput8", "Many mods hook dinput8 for custom keybinds"),
        ("xinput", "Controller mods hook xinput — changes may affect gamepad mods"),
        ("version", "DLL proxy mods commonly use version.dll — changes may break proxy loading"),
        ("winmm", "Some older mods proxy winmm.dll"),
        ("winhttp", "Some mods proxy winhttp.dll for online features"),
    ];

    for (dll, impact) in &mod_loader_dlls {
        if affected_dlls.iter().any(|d| d == *dll) {
            impacts.push(impact.to_string());
        }
    }

    // Check message for modding-relevant keywords
    if msg_lower.contains("large address aware") || msg_lower.contains("address space") {
        impacts.push("Address space changes affect 32-bit mod loaders (SKSE, F4SE legacy)".to_string());
    }
    if msg_lower.contains("dll override") || msg_lower.contains("native") {
        impacts.push("DLL override changes may affect mod proxy DLLs".to_string());
    }
    if msg_lower.contains("gamedrive") {
        impacts.push("Gamedrive affects where mods can be installed on disk".to_string());
    }

    // Classification-specific notes
    if classification == "GameConfig" && impacts.is_empty() {
        impacts.push("Game-specific config tweak — unlikely to affect mods".to_string());
    }

    impacts
}

/// Extract a Steam app ID from a GameConfig commit message.
/// e.g. "proton: Enable gamedrive for app 377160" → Some(377160)
pub fn extract_app_id(message: &str) -> Option<u32> {
    // Look for numeric app IDs in the message
    let re = regex::Regex::new(r"\b(\d{4,8})\b").ok()?;
    for cap in re.captures_iter(message) {
        if let Ok(id) = cap[1].parse::<u32>() {
            // Filter out things that aren't likely app IDs
            if id >= 1000 && id <= 99999999 {
                return Some(id);
            }
        }
    }
    None
}

/// Fetch ProtonDB ratings for a batch of app IDs. Uses a single thread with
/// short timeouts to avoid blocking for too long.
pub fn fetch_protondb_ratings(app_ids: &[u32]) -> std::collections::HashMap<u32, String> {
    let mut results = std::collections::HashMap::new();
    // Limit to 5 fetches max to avoid blocking too long
    for &app_id in app_ids.iter().take(5) {
        if results.contains_key(&app_id) {
            continue;
        }
        let url = format!("https://www.protondb.com/api/v1/reports/summaries/{app_id}.json");
        let output = Command::new("curl")
            .args(["-sf", "--max-time", "2", &url])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                let body = String::from_utf8_lossy(&out.stdout);
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(tier) = parsed.get("tier").and_then(|t| t.as_str()) {
                        results.insert(app_id, tier.to_string());
                    }
                }
            }
        }
    }
    results
}

/// Analyze a batch of patches. This is the main entry point called from FFI.
pub fn analyze_patches(
    commits: &[crate::classifier::ClassifiedCommit],
    wine_source_dir: &Path,
    game_imports: &[(String, Vec<String>)],
) -> Vec<PatchAnalysis> {
    let wine_source_exists = wine_source_dir.join(".git").exists();

    // Batch ProtonDB lookups: collect all app IDs from GameConfig commits first
    let app_ids: Vec<u32> = commits
        .iter()
        .filter(|c| c.classification == crate::classifier::Classification::GameConfig)
        .filter_map(|c| extract_app_id(&c.message))
        .collect();
    let protondb_cache = fetch_protondb_ratings(&app_ids);

    commits
        .iter()
        .map(|commit| {
            let affected_files = &commit.affected_files;
            let affected_dlls = extract_affected_dlls(affected_files);

            let (applies_cleanly, conflict_files) = if wine_source_exists && !commit.diff.is_empty() {
                dry_run_check(wine_source_dir, &commit.diff)
            } else {
                (false, vec![])
            };

            let (impact, impact_reason, lines_added, lines_removed) =
                compute_impact(&commit.diff, affected_files, &commit.classification.to_string());

            let affected_games = match_affected_games(&affected_dlls, game_imports);

            let protondb_rating = if commit.classification == crate::classifier::Classification::GameConfig {
                extract_app_id(&commit.message).and_then(|id| protondb_cache.get(&id).cloned())
            } else {
                None
            };

            // Auto-adaptation analysis
            let adapt_report = crate::auto_adapter::auto_adapt(&commit.diff);

            // Modding/gaming impact
            let modding_impact = detect_modding_impact(
                &affected_dlls,
                &commit.message,
                &commit.classification.to_string(),
            );

            PatchAnalysis {
                hash: commit.hash.clone(),
                applies_cleanly: Some(applies_cleanly),
                conflict_files,
                affected_dlls,
                impact,
                impact_reason,
                lines_added,
                lines_removed,
                affected_games,
                protondb_rating,
                can_auto_adapt: adapt_report.was_adapted,
                adaptation_transform_count: adapt_report.transforms_applied.len(),
                adaptation_confidence: adapt_report.confidence,
                adaptation_warnings: adapt_report.warnings,
                modding_impact,
                suggested_action: commit.suggested_action.clone(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_affected_dlls() {
        let files = vec![
            "dlls/d3d11/device.c".to_string(),
            "dlls/d3d11/texture.c".to_string(),
            "dlls/ntdll/sync.c".to_string(),
            "server/thread.c".to_string(),
            "README.md".to_string(),
        ];
        let dlls = extract_affected_dlls(&files);
        assert!(dlls.contains(&"d3d11".to_string()));
        assert!(dlls.contains(&"ntdll".to_string()));
        assert!(dlls.contains(&"wine-server".to_string()));
        assert_eq!(dlls.len(), 3);
    }

    #[test]
    fn test_compute_impact_small() {
        let diff = "+line1\n+line2\n-line3\n";
        let files = vec!["dlls/d3d11/x.c".to_string()];
        let (impact, _, added, removed) = compute_impact(diff, &files, "WineApiFix");
        assert_eq!(impact, "low risk");
        assert_eq!(added, 2);
        assert_eq!(removed, 1);
    }

    #[test]
    fn test_compute_impact_hot_path() {
        let diff = "+mutex_lock(&cs);\n+critical_section_enter();\n";
        let files = vec!["dlls/ntdll/sync.c".to_string()];
        let (impact, _, _, _) = compute_impact(diff, &files, "WineApiFix");
        assert_eq!(impact, "high risk");
    }

    #[test]
    fn test_match_affected_games() {
        let dlls = vec!["d3d11".to_string(), "dxgi".to_string()];
        let games = vec![
            ("Fallout 4".to_string(), vec!["d3d11.dll".to_string(), "kernel32.dll".to_string()]),
            ("Skyrim".to_string(), vec!["d3d9.dll".to_string()]),
            ("Hogwarts".to_string(), vec!["D3D11.dll".to_string(), "dxgi.dll".to_string()]),
        ];
        let matched = match_affected_games(&dlls, &games);
        assert!(matched.contains(&"Fallout 4".to_string()));
        assert!(matched.contains(&"Hogwarts".to_string()));
        assert!(!matched.contains(&"Skyrim".to_string()));
    }

    #[test]
    fn test_extract_app_id() {
        assert_eq!(extract_app_id("proton: Fix for app 377160"), Some(377160));
        assert_eq!(extract_app_id("proton: Bump version."), None);
        assert_eq!(extract_app_id("Enable gamedrive for 1245620"), Some(1245620));
    }

    #[test]
    fn test_compute_impact_build_system() {
        let diff = "+lots\n+of\n+changes\n".repeat(200);
        let files: Vec<String> = (0..20).map(|i| format!("file{i}.mk")).collect();
        let (impact, _, _, _) = compute_impact(&diff, &files, "BuildSystem");
        assert_eq!(impact, "low risk");
    }
}
