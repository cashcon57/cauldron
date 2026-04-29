#!/usr/bin/env python3
"""
Frame-level flicker analysis.

Given a recorded session video, extracts every Nth frame and computes the
mean absolute pixel difference between consecutive frames. Outputs a CSV
plus summary statistics.

Flicker pattern: short bursts of inter-frame difference far above the
typical-motion baseline, often at fixed intervals tied to compositor
refresh.

Usage:
    python3 analyze_frames.py <video> [--fps N] [--out path.csv]

Requires:
    ffmpeg on PATH
    pip3 install --user pillow numpy
"""

from __future__ import annotations

import argparse
import csv
import shutil
import statistics
import subprocess
import sys
from pathlib import Path


def require_ffmpeg() -> None:
    if shutil.which("ffmpeg") is None:
        sys.exit("ffmpeg not found on PATH. Install with: brew install ffmpeg")


def require_libs():
    try:
        from PIL import Image  # noqa: F401
        import numpy  # noqa: F401
    except ImportError:
        sys.exit("Missing dependencies. Install with: pip3 install --user pillow numpy")


def extract_frames(video: Path, out_dir: Path, fps: float) -> list[Path]:
    out_dir.mkdir(parents=True, exist_ok=True)
    # Clean any stale frames from a previous run.
    for old in out_dir.glob("frame_*.png"):
        old.unlink()
    cmd = [
        "ffmpeg",
        "-y",
        "-i",
        str(video),
        "-vf",
        f"fps={fps}",
        str(out_dir / "frame_%05d.png"),
    ]
    result = subprocess.run(cmd, capture_output=True)
    if result.returncode != 0:
        sys.exit(f"ffmpeg failed:\n{result.stderr.decode(errors='ignore')}")
    return sorted(out_dir.glob("frame_*.png"))


def pixel_diff(frame_a: Path, frame_b: Path) -> float:
    """Mean absolute pixel difference between two frames, normalized to [0, 1]."""
    from PIL import Image
    import numpy as np

    a = np.asarray(Image.open(frame_a).convert("RGB"), dtype=np.int16)
    b = np.asarray(Image.open(frame_b).convert("RGB"), dtype=np.int16)
    return float(np.abs(a - b).mean()) / 255.0


def summarize(diffs: list[float]) -> dict[str, float]:
    if not diffs:
        return {"n": 0}
    sorted_diffs = sorted(diffs, reverse=True)
    top10_count = max(1, len(diffs) // 10)
    return {
        "n": len(diffs),
        "mean": statistics.mean(diffs),
        "stdev": statistics.stdev(diffs) if len(diffs) > 1 else 0.0,
        "max": max(diffs),
        "p50": statistics.median(diffs),
        "p90": sorted(diffs)[int(0.9 * len(diffs))],
        "top10_mean": statistics.mean(sorted_diffs[:top10_count]),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("video", help="path to the session video (.mov / .mp4)")
    ap.add_argument(
        "--fps",
        type=float,
        default=10,
        help="frames per second to sample (default: 10)",
    )
    ap.add_argument("--out", default=None, help="CSV output path (default: <video>.diff.csv)")
    args = ap.parse_args()

    require_ffmpeg()
    require_libs()

    video = Path(args.video)
    if not video.exists():
        sys.exit(f"video not found: {video}")

    out_csv = Path(args.out) if args.out else video.with_suffix(".diff.csv")
    frames_dir = video.parent / (video.stem + "_frames")

    print(f"Extracting frames at {args.fps} fps from {video.name} ...")
    frames = extract_frames(video, frames_dir, fps=args.fps)
    print(f"  -> {len(frames)} frames in {frames_dir}")

    if len(frames) < 2:
        sys.exit("Not enough frames to compute pairwise diffs.")

    print("Computing pairwise pixel diffs ...")
    diffs: list[float] = []
    for i in range(1, len(frames)):
        diffs.append(pixel_diff(frames[i - 1], frames[i]))

    with out_csv.open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["frame_idx", "diff_normalized"])
        for i, d in enumerate(diffs, start=1):
            w.writerow([i, f"{d:.6f}"])

    stats = summarize(diffs)
    print()
    print(f"=== {video.name} ===")
    for k, v in stats.items():
        if isinstance(v, float):
            print(f"  {k:>12}: {v:.6f}")
        else:
            print(f"  {k:>12}: {v}")
    print()
    print(f"  CSV: {out_csv}")
    print()
    print("Interpretation:")
    print("  - 'mean' is the typical inter-frame change (motion + flicker combined).")
    print("  - 'top10_mean' weights the most-changed frames; this is where flicker")
    print("    spikes show up. A meaningfully lower top10_mean on the patched run")
    print("    vs the baseline run is consistent with reduced flicker.")
    print("  - 'p90' is the 90th percentile diff. Sharp differences between p50")
    print("    and p90 indicate intermittent, large changes (flicker-like).")

    return 0


if __name__ == "__main__":
    sys.exit(main())
