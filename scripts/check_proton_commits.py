#!/usr/bin/env python3
"""
Check recent Proton commits and classify them by transferability
to the Cauldron project.

Classifies commits based on which files they touch:
  - High:   wine patches, DXVK/VKD3D shader translation, config mappings
  - Medium: build system, compatibility flags, launcher scripts
  - Low:    CI, docs, Steam-specific runtime, Linux-only plumbing

Usage:
    python3 check_proton_commits.py --repo-path /path/to/proton
    python3 check_proton_commits.py --repo-path /path/to/proton --since 14 --output json
"""

import argparse
import json
import subprocess
import sys
from collections import defaultdict
from datetime import datetime, timedelta


# Path-based classification rules. Order matters: first match wins.
CLASSIFICATION_RULES = [
    # High transferability
    ("High", ["patches/wine/", "wine/", "dxvk/", "vkd3d-proton/", "d3d"]),
    ("High", ["default_compat_config", "proton_config", "compatibilitytool"]),
    # Medium transferability
    ("Medium", ["Makefile", "build/", "configure", "setup.py"]),
    ("Medium", ["proton", "filelock", "user_settings"]),
    # Low transferability
    ("Low", [".github/", "docs/", "README", "LICENSE", "CHANGELOG"]),
    ("Low", ["steam", "pressure-vessel", "container", "sniper"]),
]


def classify_commit(files_changed: list[str]) -> str:
    """Classify a commit based on the files it changed."""
    for level, patterns in CLASSIFICATION_RULES:
        for f in files_changed:
            for pattern in patterns:
                if pattern.lower() in f.lower():
                    return level
    return "Medium"  # default when no pattern matches


def get_commits(repo_path: str, since_days: int) -> list[dict]:
    """Get recent commits from the repo using git log."""
    since_date = (datetime.now() - timedelta(days=since_days)).strftime("%Y-%m-%d")

    # Get commit hashes and subjects
    log_result = subprocess.run(
        [
            "git", "-C", repo_path, "log",
            f"--since={since_date}",
            "--pretty=format:%H|%as|%s",
        ],
        capture_output=True,
        text=True,
    )

    if log_result.returncode != 0:
        print(f"ERROR: git log failed: {log_result.stderr}", file=sys.stderr)
        sys.exit(1)

    commits = []
    for line in log_result.stdout.strip().splitlines():
        if not line:
            continue
        parts = line.split("|", 2)
        if len(parts) < 3:
            continue
        sha, date, subject = parts

        # Get files changed by this commit
        diff_result = subprocess.run(
            [
                "git", "-C", repo_path, "diff-tree",
                "--no-commit-id", "-r", "--name-only", sha,
            ],
            capture_output=True,
            text=True,
        )
        files = [f for f in diff_result.stdout.strip().splitlines() if f]

        classification = classify_commit(files)
        commits.append({
            "sha": sha[:12],
            "date": date,
            "subject": subject,
            "files_changed": len(files),
            "classification": classification,
        })

    return commits


def format_text(commits: list[dict]) -> str:
    """Format commit summary as human-readable text."""
    if not commits:
        return "No commits found in the specified time range."

    counts = defaultdict(int)
    for c in commits:
        counts[c["classification"]] += 1

    lines = [
        f"Proton Commit Summary",
        f"{'=' * 40}",
        f"Total commits: {len(commits)}",
        f"  High transferability:   {counts.get('High', 0)}",
        f"  Medium transferability: {counts.get('Medium', 0)}",
        f"  Low transferability:    {counts.get('Low', 0)}",
        "",
    ]

    high_commits = [c for c in commits if c["classification"] == "High"]
    if high_commits:
        lines.append("High Transferability Commits:")
        lines.append("-" * 40)
        for c in high_commits:
            lines.append(f"  [{c['date']}] {c['sha']} {c['subject']}")
    else:
        lines.append("No high-transferability commits found.")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(
        description="Check and classify recent Proton commits."
    )
    parser.add_argument(
        "--repo-path",
        required=True,
        help="Path to the local Proton git repository",
    )
    parser.add_argument(
        "--since",
        type=int,
        default=7,
        help="Number of days to look back (default: 7)",
    )
    parser.add_argument(
        "--output",
        choices=["json", "text"],
        default="text",
        help="Output format (default: text)",
    )
    args = parser.parse_args()

    commits = get_commits(args.repo_path, args.since)

    if args.output == "json":
        json.dump(commits, sys.stdout, indent=2)
        print()
    else:
        print(format_text(commits))


if __name__ == "__main__":
    main()
