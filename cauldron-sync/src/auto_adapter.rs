//! Automated patch adaptation: transforms Linux-specific code patterns to macOS equivalents.
//!
//! Applies mechanical source transformations for well-understood Linux→macOS API mappings.
//! Returns the adapted diff and a report of what was changed and why.

use std::sync::LazyLock;

/// A single adaptation transform rule.
struct TransformRule {
    /// What Linux API pattern to look for in the diff.
    linux_pattern: &'static str,
    /// The macOS replacement.
    macos_replacement: &'static str,
    /// Human-readable description of what this transform does.
    description: &'static str,
    /// Which category this belongs to.
    category: &'static str,
}

/// Result of attempting to adapt a patch.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AdaptationReport {
    /// The adapted diff (with Linux patterns replaced by macOS equivalents).
    pub adapted_diff: String,
    /// Whether any transforms were applied.
    pub was_adapted: bool,
    /// List of transforms that were applied.
    pub transforms_applied: Vec<TransformApplied>,
    /// Warnings about patterns that were detected but couldn't be auto-adapted.
    pub warnings: Vec<String>,
    /// Overall confidence: "high", "medium", "low"
    pub confidence: String,
}

/// A single transform that was applied to the diff.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TransformApplied {
    pub linux_api: String,
    pub macos_api: String,
    pub description: String,
    pub occurrences: usize,
}

// Tier 1: Direct string replacements (high confidence)
static TIER1_TRANSFORMS: LazyLock<Vec<TransformRule>> = LazyLock::new(|| vec![
    TransformRule {
        linux_pattern: "/proc/self/exe",
        macos_replacement: "/* macOS: _NSGetExecutablePath() */",
        description: "Linux /proc/self/exe → macOS _NSGetExecutablePath()",
        category: "proc_filesystem",
    },
    TransformRule {
        linux_pattern: "/proc/self/maps",
        macos_replacement: "/* macOS: mach_vm_region() */",
        description: "Linux /proc/self/maps → macOS mach_vm_region()",
        category: "proc_filesystem",
    },
    TransformRule {
        linux_pattern: "prctl(PR_SET_NAME",
        macos_replacement: "pthread_setname_np(",
        description: "Linux prctl(PR_SET_NAME) → macOS pthread_setname_np()",
        category: "thread_naming",
    },
    TransformRule {
        linux_pattern: "PR_SET_NAME",
        macos_replacement: "/* macOS: pthread_setname_np() */",
        description: "PR_SET_NAME constant → pthread_setname_np()",
        category: "thread_naming",
    },
    TransformRule {
        linux_pattern: "#include <linux/futex.h>",
        macos_replacement: "#include <os/lock.h> /* macOS: MSync via os_unfair_lock / __ulock */",
        description: "Linux futex header → macOS os/lock.h (MSync)",
        category: "synchronization",
    },
    TransformRule {
        linux_pattern: "#include <sys/eventfd.h>",
        macos_replacement: "#include <sys/event.h> /* macOS: kqueue EVFILT_USER replaces eventfd */",
        description: "Linux eventfd header → macOS kqueue",
        category: "event_notification",
    },
    TransformRule {
        linux_pattern: "#include <sys/epoll.h>",
        macos_replacement: "#include <sys/event.h> /* macOS: kqueue replaces epoll */",
        description: "Linux epoll header → macOS kqueue",
        category: "io_multiplexing",
    },
    TransformRule {
        linux_pattern: "#include <sys/inotify.h>",
        macos_replacement: "#include <CoreServices/CoreServices.h> /* macOS: FSEvents replaces inotify */",
        description: "Linux inotify header → macOS FSEvents",
        category: "filesystem_monitoring",
    },
    TransformRule {
        linux_pattern: "#include <sys/signalfd.h>",
        macos_replacement: "#include <sys/event.h> /* macOS: kqueue EVFILT_SIGNAL replaces signalfd */",
        description: "Linux signalfd header → macOS kqueue EVFILT_SIGNAL",
        category: "signal_handling",
    },
]);

// Tier 2: Ifdef-based adaptations (medium confidence)
static IFDEF_LINUX_PATTERN: LazyLock<regex::Regex> = LazyLock::new(||
    regex::Regex::new(r"#ifdef\s+__linux__").unwrap()
);

// Patterns that indicate a patch needs manual review (can't auto-adapt)
static MANUAL_REVIEW_PATTERNS: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| vec![
    ("io_uring", "io_uring → dispatch_io requires significant restructuring"),
    ("clone3(", "clone3() → posix_spawn() has complex flag mapping"),
    ("memfd_create", "memfd_create() → shm_open()+ftruncate() needs lifecycle management"),
    ("CLONE_NEWNS", "Linux namespaces have no macOS equivalent"),
    ("mount(", "Linux mount() syscall differs significantly from macOS"),
    ("splice(", "splice() → sendfile() on macOS, different semantics"),
    ("tee(", "tee() pipe duplication has no direct macOS equivalent"),
]);

/// Attempt to automatically adapt a Linux-specific diff for macOS.
pub fn auto_adapt(diff: &str) -> AdaptationReport {
    if diff.is_empty() {
        return AdaptationReport {
            adapted_diff: String::new(),
            was_adapted: false,
            transforms_applied: Vec::new(),
            warnings: Vec::new(),
            confidence: "high".to_string(),
        };
    }

    let mut adapted = diff.to_string();
    let mut transforms_applied = Vec::new();
    let mut warnings = Vec::new();

    // Apply Tier 1 transforms (direct replacements)
    for rule in TIER1_TRANSFORMS.iter() {
        let count = adapted.matches(rule.linux_pattern).count();
        if count > 0 {
            adapted = adapted.replace(rule.linux_pattern, rule.macos_replacement);
            transforms_applied.push(TransformApplied {
                linux_api: rule.linux_pattern.to_string(),
                macos_api: rule.macos_replacement.to_string(),
                description: rule.description.to_string(),
                occurrences: count,
            });
        }
    }

    // Apply Tier 2: Add macOS ifdef blocks alongside linux ifdefs
    // Transform `#ifdef __linux__` to `#if defined(__linux__) || defined(__APPLE__)`
    // Only when the block contains code that we've already adapted above
    let ifdef_count = IFDEF_LINUX_PATTERN.find_iter(&adapted).count();
    if ifdef_count > 0 && !transforms_applied.is_empty() {
        adapted = IFDEF_LINUX_PATTERN.replace_all(
            &adapted,
            "#if defined(__linux__) || defined(__APPLE__)"
        ).to_string();
        transforms_applied.push(TransformApplied {
            linux_api: "#ifdef __linux__".to_string(),
            macos_api: "#if defined(__linux__) || defined(__APPLE__)".to_string(),
            description: "Extended Linux-only ifdef to include macOS".to_string(),
            occurrences: ifdef_count,
        });
    }

    // Check for manual-review patterns (Tier 3)
    for (pattern, warning) in MANUAL_REVIEW_PATTERNS.iter() {
        if diff.contains(pattern) {
            warnings.push(format!("{pattern}: {warning}"));
        }
    }

    let was_adapted = !transforms_applied.is_empty();

    let confidence = if warnings.is_empty() && was_adapted {
        "high".to_string()
    } else if was_adapted && warnings.len() <= 1 {
        "medium".to_string()
    } else if !warnings.is_empty() {
        "low".to_string()
    } else {
        "high".to_string() // No Linux-specific patterns found at all
    };

    AdaptationReport {
        adapted_diff: adapted,
        was_adapted,
        transforms_applied,
        warnings,
        confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proc_self_exe_replacement() {
        let diff = r#"+    path = "/proc/self/exe";
+    readlink(path, buf, sizeof(buf));"#;
        let report = auto_adapt(diff);
        assert!(report.was_adapted);
        assert!(!report.adapted_diff.contains("/proc/self/exe"));
        assert!(report.adapted_diff.contains("_NSGetExecutablePath"));
        assert_eq!(report.transforms_applied.len(), 1);
        assert_eq!(report.confidence, "high");
    }

    #[test]
    fn test_futex_header_replacement() {
        let diff = r#"+#include <linux/futex.h>
+    futex_wait(&addr, val);"#;
        let report = auto_adapt(diff);
        assert!(report.was_adapted);
        assert!(report.adapted_diff.contains("os/lock.h"));
        assert!(report.adapted_diff.contains("MSync"));
    }

    #[test]
    fn test_ifdef_linux_expansion() {
        let diff = r#"+#include <linux/futex.h>
+#ifdef __linux__
+    do_linux_thing();
+#endif"#;
        let report = auto_adapt(diff);
        assert!(report.was_adapted);
        assert!(report.adapted_diff.contains("defined(__APPLE__)"));
    }

    #[test]
    fn test_manual_review_warning() {
        let diff = "+    io_uring_setup(entries, &params);";
        let report = auto_adapt(diff);
        assert!(!report.warnings.is_empty());
        assert!(report.warnings[0].contains("io_uring"));
        assert_eq!(report.confidence, "low");
    }

    #[test]
    fn test_no_linux_patterns() {
        let diff = "+    printf(\"hello world\\n\");";
        let report = auto_adapt(diff);
        assert!(!report.was_adapted);
        assert!(report.warnings.is_empty());
        assert_eq!(report.confidence, "high");
    }

    #[test]
    fn test_multiple_transforms() {
        let diff = r#"+#include <sys/epoll.h>
+#include <sys/eventfd.h>
+    epoll_create1(0);
+    eventfd(0, EFD_NONBLOCK);"#;
        let report = auto_adapt(diff);
        assert!(report.was_adapted);
        assert!(report.transforms_applied.len() >= 2);
    }

    #[test]
    fn test_prctl_replacement() {
        let diff = "+    prctl(PR_SET_NAME, \"worker\");";
        let report = auto_adapt(diff);
        assert!(report.was_adapted);
        assert!(report.adapted_diff.contains("pthread_setname_np"));
    }

    #[test]
    fn test_clone3_warning() {
        let diff = "+    clone3(&args, sizeof(args));";
        let report = auto_adapt(diff);
        assert!(report.warnings.iter().any(|w| w.contains("clone3")));
    }
}
