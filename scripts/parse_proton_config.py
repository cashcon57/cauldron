#!/usr/bin/env python3
"""
Parse Proton's default_compat_config() function to extract per-game
compatibility flags.

Proton's main `proton` script contains a function called
default_compat_config() that returns a dict mapping Steam app IDs to
configuration strings (e.g. "noopwr gamedrive"). This script extracts
those mappings and outputs them as JSON so that cauldron-sync can
ingest them into the compatibility database.

Usage:
    python3 parse_proton_config.py /path/to/proton/proton
"""

import argparse
import json
import re
import sys
from pathlib import Path


def parse_compat_config(proton_script: str) -> list[dict]:
    """Extract app_id -> config mappings from default_compat_config().

    Looks for patterns like:
        "12345": "flag1 flag2",
    inside the default_compat_config() function body.
    """
    # Find the function body of default_compat_config
    func_pattern = re.compile(
        r'def\s+default_compat_config\s*\(\s*\)\s*:.*?(?=\ndef\s|\Z)',
        re.DOTALL,
    )
    func_match = func_pattern.search(proton_script)
    if not func_match:
        print(
            "ERROR: Could not find default_compat_config() in the provided script.",
            file=sys.stderr,
        )
        sys.exit(1)

    func_body = func_match.group(0)

    # Match entries like "12345": "flag1 flag2 key=val",
    entry_pattern = re.compile(
        r'["\'](\d+)["\']\s*:\s*["\']([^"\']*)["\']'
    )

    results = []
    for match in entry_pattern.finditer(func_body):
        app_id = int(match.group(1))
        config_str = match.group(2).strip()

        flags = []
        overrides = {}
        for token in config_str.split():
            if "=" in token:
                key, _, value = token.partition("=")
                overrides[key] = value
            else:
                flags.append(token)

        results.append({
            "app_id": app_id,
            "flags": flags,
            "overrides": overrides,
        })

    return results


def main():
    parser = argparse.ArgumentParser(
        description="Parse Proton's default_compat_config() and output JSON."
    )
    parser.add_argument(
        "proton_script",
        type=str,
        help="Path to Proton's 'proton' Python script",
    )
    args = parser.parse_args()

    script_path = Path(args.proton_script)
    if not script_path.is_file():
        print(f"ERROR: File not found: {script_path}", file=sys.stderr)
        sys.exit(1)

    content = script_path.read_text(encoding="utf-8")
    configs = parse_compat_config(content)

    print(f"Parsed {len(configs)} app config entries.", file=sys.stderr)
    json.dump(configs, sys.stdout, indent=2)
    print()  # trailing newline


if __name__ == "__main__":
    main()
