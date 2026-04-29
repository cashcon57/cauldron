# Wine winemac.drv Flicker A/B Test

Empirical test harness for `patches/cauldron/0003-winemac-drv-reduce-compositor-flicker.patch`.

The patch claims to fix full-screen flickering on macOS Wine. As of this writing the patch IS committed to the wine fork (commit `641f702 cauldron: winemac.drv: Reduce compositor flicker via atomic ...`) and IS compiled into the deployed Cauldron Wine runtime at `~/Library/Cauldron/wine`. What's missing is empirical A/B evidence — the patch has never been compared to a known-baseline build with the same source state, on the same game, in the same conditions. This harness fills that gap.

## What this does

1. Builds two variants of `winemac.drv` from the existing Cauldron Wine source tree:
   - **baseline** — no flicker patch
   - **patched** — `patches/cauldron/0003-...patch` applied
2. Lets you swap between them in the deployed Cauldron Wine runtime (`~/Library/Cauldron/wine`) by copying the corresponding binaries into place.
3. Records a 30-second screen capture of a game running under each variant.
4. Runs a frame-diff analyzer that produces a per-frame pixel-difference CSV. Flicker shows up as periodic spikes in inter-frame difference.

The recordings are the primary deliverable. Visual inspection by a human is the gold standard for flicker detection — frame diff is corroborating evidence, not a substitute.

## Prerequisites

- A working Cauldron Wine build at `/Users/cashconway/cauldron/build/wine` (autotools build dir already configured and built once).
- A working Cauldron Wine runtime at `~/Library/Cauldron/wine`.
- A game installed in a Cauldron bottle that you believe exhibits flicker. Per `patches/FLICKER_FIX_SPEC.md`, candidates are: Tainted Grail, Hogwarts Legacy, Skyrim SE, Fallout 4. Pick whichever is available.
- macOS Screen Recording permission granted to **Terminal** (or whichever app runs `screencapture -V`). System Settings → Privacy & Security → Screen Recording.
- `ffmpeg` and Python 3 with `pillow` + `numpy` (for `analyze_frames.py`):

  ```sh
  brew install ffmpeg
  pip3 install --user pillow numpy
  ```

## Workflow

```sh
cd /Users/cashconway/cauldron

# One-shot orchestrator (recommended):
./tests/flicker/run_all.sh <bottle-id> <wine-path-to-exe> [duration_seconds]

# Or run each step manually:
./tests/flicker/build_variants.sh
./tests/flicker/swap_variant.sh baseline
./tests/flicker/record_session.sh baseline <bottle-id> <exe> 30
./tests/flicker/swap_variant.sh patched
./tests/flicker/record_session.sh patched <bottle-id> <exe> 30
python3 ./tests/flicker/analyze_frames.py <out_dir>/baseline.mov
python3 ./tests/flicker/analyze_frames.py <out_dir>/patched.mov
```

Recordings and metadata land in `/Users/Shared/localci-logs/wine-flicker/<timestamp>/`.

## How to interpret results

- **Visual:** open `baseline.mov` and `patched.mov` side by side. The flicker is a full-screen brightness/content discontinuity at compositor refresh boundaries. If `patched.mov` looks stable while `baseline.mov` flickers, the patch works.
- **Frame diff CSV:** sustained low values with occasional huge spikes → flicker. Sustained moderate values with no spikes → smooth animation. Compare the spike rate and amplitude between the two CSVs.
- **Top-10% mean:** the analyzer prints this. A meaningfully lower top-10% mean on the patched run vs the baseline run is consistent with reduced flicker.

## What the test cannot tell you

- Whether the flicker is caused by something other than `winemac.drv` (the patch only modifies `dlls/winemac.drv/cocoa_window.m`; if the real issue lives elsewhere — e.g., DXMT, the bottle's RetinaMode setting, or `surface.c::macdrv_window_surface_flush` which the spec calls "critical" but which the current patch does NOT touch — the test will show no improvement and the patch is, in fact, insufficient).
- Whether other regressions are introduced (mouse cursor, fullscreen transitions, multi-monitor behavior). Manual smoke-test these.
- Performance impact (CATransaction wrap + animation suppression has zero cost, but worth checking FPS isn't reduced).

## Cleanup

```sh
./tests/flicker/swap_variant.sh baseline   # leave runtime unmodified
```

The `variants/` subdirectory holds built artifacts and is `.gitignore`d.

## Open question this test does NOT answer

Whether the patch matches what wine-staging ships or what CrossOver ships. Per investigation in this branch:

- **wine-staging** has a no-flicker patch but it doesn't apply cleanly to Wine 11.6 (per `patches/FLICKER_FIX_SPEC.md`). Our 0003 is a partial rebase.
- **CrossOver** does NOT use `CATransaction` in their published `winemac.drv` source (verified across `3Shain/winecx` and `marzent/winecx` mirrors, branches `crossover-wine` and `cx-22`). Their flicker reduction uses some other mechanism we haven't identified.

If this test confirms our patch eliminates flicker, that's standalone Cauldron value worth contributing upstream. If it doesn't, we need to revisit the spec's Part 1 (the `surface.c` change the spec calls critical, which 0003 omits).
