use crate::classifier::{ClassifiedCommit, Classification, Transferability};
use regex::Regex;

/// The result of analyzing a commit for macOS adaptation.
#[derive(Debug, Clone)]
pub enum AdaptationResult {
    /// Can be applied directly with no modifications.
    DirectApply(String),
    /// Requires translating a Linux mechanism to a macOS equivalent.
    NeedsTranslation {
        linux_mechanism: String,
        macos_equivalent: String,
        notes: String,
    },
    /// Should be skipped entirely.
    Skip(String),
    /// Needs human review before deciding.
    ManualReview(String),
}

/// Known mappings from Linux kernel mechanisms to macOS equivalents.
pub const KERNEL_MAPPINGS: &[(&str, &str, &str)] = &[
    (
        "futex",
        "MSync (os_unfair_lock / __ulock)",
        "macOS uses MSync via os_unfair_lock for Wine synchronization primitives",
    ),
    (
        "eventfd",
        "kqueue (EVFILT_USER)",
        "Replace eventfd-based signaling with kqueue EVFILT_USER events",
    ),
    (
        "epoll",
        "kqueue",
        "macOS kqueue is the direct equivalent of Linux epoll",
    ),
    (
        "/proc/self",
        "sysctl / dyld APIs",
        "Process introspection via sysctl(3) or _dyld_ functions instead of procfs",
    ),
    (
        "memfd_create",
        "shm_open + ftruncate",
        "Use POSIX shared memory APIs on macOS for anonymous memory-backed file descriptors",
    ),
    (
        "prctl",
        "pthread_setname_np",
        "Thread naming and signal handling via pthread APIs on macOS",
    ),
    (
        "clone3",
        "posix_spawn / pthread_create",
        "macOS does not support clone3; use posix_spawn or pthread_create",
    ),
    (
        "io_uring",
        "dispatch_io / kqueue",
        "macOS has no io_uring; use GCD dispatch_io or kqueue for async I/O",
    ),
    (
        "timerfd",
        "dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER)",
        "Use GCD timer sources as the macOS equivalent of timerfd",
    ),
    (
        "signalfd",
        "kqueue (EVFILT_SIGNAL)",
        "Use kqueue signal filters instead of signalfd on macOS",
    ),
    (
        "inotify",
        "FSEvents / kqueue (EVFILT_VNODE)",
        "File system monitoring via FSEvents framework or kqueue vnode events",
    ),
];

/// Suggest how a classified commit should be adapted for macOS.
pub fn suggest_adaptation(commit: &ClassifiedCommit) -> AdaptationResult {
    tracing::debug!(
        hash = %commit.hash,
        classification = ?commit.classification,
        transferability = ?commit.transferability,
        "Suggesting adaptation for commit"
    );
    match (&commit.classification, &commit.transferability) {
        // High transferability API/DXVK fixes can go straight in
        (Classification::WineApiFix, Transferability::High) => {
            AdaptationResult::DirectApply(format!(
                "Wine API fix '{}' is platform-independent and can be applied directly",
                truncate_message(&commit.message)
            ))
        }
        (Classification::DxvkFix, Transferability::High) => {
            AdaptationResult::DirectApply(format!(
                "DXVK fix '{}' works with MoltenVK on macOS",
                truncate_message(&commit.message)
            ))
        }
        (Classification::GameConfig, Transferability::High) => {
            AdaptationResult::DirectApply(format!(
                "Game config '{}' is directly transferable",
                truncate_message(&commit.message)
            ))
        }

        // Kernel workarounds need translation
        (Classification::KernelWorkaround, Transferability::Low) => {
            check_kernel_mappings(&commit.diff)
        }

        // Build system changes are skipped
        (Classification::BuildSystem, Transferability::None) | (_, Transferability::None) => {
            AdaptationResult::Skip(format!(
                "Build/infrastructure change '{}' is not applicable to macOS",
                truncate_message(&commit.message)
            ))
        }

        // Everything else needs manual review
        _ => AdaptationResult::ManualReview(format!(
            "Commit '{}' classified as {:?}/{:?} needs manual review",
            truncate_message(&commit.message),
            commit.classification,
            commit.transferability
        )),
    }
}

/// Check the diff content against known kernel mechanism patterns.
fn check_kernel_mappings(diff: &str) -> AdaptationResult {
    for &(linux_pattern, macos_equiv, notes) in KERNEL_MAPPINGS {
        let re = Regex::new(&format!(r"(?i){}", regex::escape(linux_pattern)))
            .expect("valid regex from known pattern");

        if re.is_match(diff) {
            tracing::info!(
                "Found kernel mapping: {} -> {}",
                linux_pattern,
                macos_equiv
            );
            return AdaptationResult::NeedsTranslation {
                linux_mechanism: linux_pattern.to_string(),
                macos_equivalent: macos_equiv.to_string(),
                notes: notes.to_string(),
            };
        }
    }

    AdaptationResult::ManualReview(
        "Kernel workaround detected but no known mapping found — manual review required".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::RawCommit;

    fn make_classified(
        classification: Classification,
        transferability: Transferability,
        diff: &str,
    ) -> ClassifiedCommit {
        ClassifiedCommit {
            hash: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "dev".to_string(),
            timestamp: "2024-01-01".to_string(),
            diff: diff.to_string(),
            affected_files: vec![],
            classification,
            transferability,
            suggested_action: String::new(),
        }
    }

    #[test]
    fn test_suggest_wine_api_fix() {
        let commit = make_classified(Classification::WineApiFix, Transferability::High, "");
        match suggest_adaptation(&commit) {
            AdaptationResult::DirectApply(msg) => assert!(msg.contains("Wine API fix")),
            other => panic!("Expected DirectApply, got {:?}", other),
        }
    }

    #[test]
    fn test_suggest_dxvk_fix() {
        let commit = make_classified(Classification::DxvkFix, Transferability::High, "");
        match suggest_adaptation(&commit) {
            AdaptationResult::DirectApply(msg) => assert!(msg.contains("DXVK fix")),
            other => panic!("Expected DirectApply, got {:?}", other),
        }
    }

    #[test]
    fn test_suggest_game_config() {
        let commit = make_classified(Classification::GameConfig, Transferability::High, "");
        match suggest_adaptation(&commit) {
            AdaptationResult::DirectApply(msg) => assert!(msg.contains("Game config")),
            other => panic!("Expected DirectApply, got {:?}", other),
        }
    }

    #[test]
    fn test_suggest_kernel_workaround_with_futex() {
        let commit = make_classified(
            Classification::KernelWorkaround,
            Transferability::Low,
            "some code using futex_wait",
        );
        match suggest_adaptation(&commit) {
            AdaptationResult::NeedsTranslation {
                linux_mechanism,
                macos_equivalent,
                ..
            } => {
                assert_eq!(linux_mechanism, "futex");
                assert!(macos_equivalent.contains("MSync"));
            }
            other => panic!("Expected NeedsTranslation, got {:?}", other),
        }
    }

    #[test]
    fn test_suggest_kernel_workaround_with_epoll() {
        let commit = make_classified(
            Classification::KernelWorkaround,
            Transferability::Low,
            "epoll_create() call",
        );
        match suggest_adaptation(&commit) {
            AdaptationResult::NeedsTranslation {
                linux_mechanism,
                macos_equivalent,
                ..
            } => {
                assert_eq!(linux_mechanism, "epoll");
                assert!(macos_equivalent.contains("kqueue"));
            }
            other => panic!("Expected NeedsTranslation, got {:?}", other),
        }
    }

    #[test]
    fn test_suggest_build_system_skip() {
        let commit = make_classified(Classification::BuildSystem, Transferability::None, "");
        match suggest_adaptation(&commit) {
            AdaptationResult::Skip(msg) => assert!(msg.contains("not applicable")),
            other => panic!("Expected Skip, got {:?}", other),
        }
    }

    #[test]
    fn test_suggest_unknown_manual_review() {
        let commit = make_classified(Classification::Unknown, Transferability::Medium, "");
        match suggest_adaptation(&commit) {
            AdaptationResult::ManualReview(_) => {}
            other => panic!("Expected ManualReview, got {:?}", other),
        }
    }
}

/// Truncate a commit message to its first line, max 80 chars.
fn truncate_message(msg: &str) -> String {
    let first_line = msg.lines().next().unwrap_or(msg);
    if first_line.len() > 80 {
        format!("{}...", &first_line[..77])
    } else {
        first_line.to_string()
    }
}
