# winemac.drv Flicker Fix — Patch Specification

**Priority:** P0 — affects every game on macOS
**Status:** Not yet implemented. Wine-staging has a version but it conflicts with Wine 11.6.
**Target:** Wine 11.6 (Cauldron fork), `dlls/winemac.drv/`

---

## The Problem

Every game launched via Wine on macOS exhibits full-screen flickering. The flicker is:
- **Backend-independent** — occurs with DXMT, DXVK+MoltenVK, and D3DMetal
- **Window-mode-independent** — occurs in borderless windowed AND exclusive fullscreen
- **Game-independent** — confirmed in Fallout 4, Hogwarts Legacy, Skyrim SE, Tainted Grail
- **Env-var-resistant** — `iPresentInterval=0`, `__GL_SYNC_TO_VBLANK=1`, `MESA_VK_WSI_PRESENT_MODE=fifo` do not fix it

This proves the flicker is at the **Wine window surface layer** (`winemac.drv`), not in any graphics translation backend.

## Root Cause

`dlls/winemac.drv/surface.c` — the `macdrv_window_surface_flush()` function.

When Wine renders a frame, it:
1. Writes pixel data into the window surface's backing store (a `CGBitmapContext`)
2. Creates a `CGImage` from the bitmap
3. Sets the `CGImage` as the content of the window's `CALayer` via `[layer setContents:]`

**The race condition:** Step 3 happens on the Wine thread, but the macOS window server compositor reads `CALayer.contents` on its own display refresh cycle. If the compositor reads between steps 2 and 3, or during step 1 of the *next* frame, it picks up a partially-rendered frame → visible flicker/tearing.

On Linux, this isn't a problem because X11/Wayland have explicit frame submission semantics. macOS's `CALayer` content model is designed for UI animations, not 60fps game rendering — it assumes content updates are infrequent and can be interpolated.

## The Fix

### Part 1: Atomic CATransaction Wrapping (surface.c)

Wrap the surface flush in a `CATransaction` to make the layer content update atomic from the compositor's perspective:

```c
// In macdrv_window_surface_flush() — after the CGImage is created,
// before setting layer contents:

[CATransaction begin];
[CATransaction setDisableActions:YES];  // No implicit animations
[CATransaction setAnimationDuration:0]; // Immediate

[layer setContents:(id)





cgimage];

[CATransaction commit];  // Compositor sees this as one atomic update
```

`setDisableActions:YES` is critical — without it, Core Animation inserts a 0.25s implicit animation on every `contents` change, which causes a different (smoother but still wrong) kind of visual artifact.

### Part 2: Disable Implicit Layer Animations (cocoa_window.m)

When creating Wine windows, suppress all implicit `CALayer` animations:

```objc
// In the WineWindow initialization or -[WineContentView updateLayer]:

CALayer *layer = [contentView layer];
layer.actions = @{
    @"contents": [NSNull null],
    @"bounds":   [NSNull null],
    @"position": [NSNull null],
};
```

This prevents Core Animation from interpolating between old and new frame contents during the compositor's render pass.

### Part 3: Disable Window Animation Behavior (cocoa_window.m)

```objc
// In WineWindow creation:
[window setAnimationBehavior:NSWindowAnimationBehaviorNone];
```

This prevents the window server from inserting its own frame interpolation on Wine-managed windows (e.g., during resize, space transitions, or Mission Control).

### Part 4: CVDisplayLink Synchronization (optional, higher performance)

For maximum correctness, tie surface flushes to the display's vsync signal:

```c
// Create a CVDisplayLink for the window's screen
CVDisplayLinkCreateWithActiveCGDisplays(&displayLink);
CVDisplayLinkSetOutputCallback(displayLink, &displayLinkCallback, surface);
CVDisplayLinkStart(displayLink);

// In the callback — signal the Wine thread that it's safe to present
static CVReturn displayLinkCallback(CVDisplayLinkRef displayLink,
    const CVTimeStamp *now, const CVTimeStamp *
    
    
    outputTime,
    CVOptionFlags flagsIn, CVOptionFlags *flagsOut, void *context)
{
    // Signal the surface that the compositor has finished scanning out
    // the previous frame and it's safe to update CALayer.contents
    dispatch_semaphore_signal(
    
    
    
    
    surface->
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    vsync_
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    semaphore);
    return kCVReturnSuccess;
}

// In macdrv_window_surface_flush():
dispatch_semaphore_wait(surface->vsync_semaphore, timeout);
// ... then do the CATransaction-wrapped setContents:
```

Part 4 is optional but provides the most flicker-free experience. Parts 1-3 are the minimum viable fix.

## Files to Modify

| File | Changes |
|------|---------|
| `dlls/winemac.drv/surface.c` | Wrap `setContents:` in CATransaction (Part 1) |
| `dlls/winemac.drv/cocoa_window.m` | Disable implicit animations (Part 2), window animation behavior (Part 3) |
| `dlls/winemac.drv/macdrv_cocoa.h` | Add function declarations if needed |
| `dlls/winemac.drv/cocoa_opengl.m` | Same CATransaction fix for OpenGL surface path (if separate) |

## Wine-Staging Reference

Wine-staging's patchset is called `winemac.drv-no-flicker-patch`. It was written against Wine ~9.x and conflicts with Wine 10.0+. The core idea is the same (CATransaction wrapping) but the function signatures and surface struct layout have changed in Wine 11.x. A manual rebase is required.

The wine-staging patches can be found at:
```
https://github.com/wine-staging/wine-staging/tree/master/patches/winemac.drv-no-flicker-patch
```

## How to Build

After applying this patch to the Wine 11.6 source:

```bash
cd /Users/cashconway/cauldron
make wine-init          # Clone Wine 11.6, apply all patches including this one
make wine-build         # Build (~1 hour on M-series)
```

The resulting Wine binary at `build/wine-dist/bin/wine64` will have the fix compiled in.

## Testing

1. Launch any DX11 game (Fallout 4, Skyrim SE) via Cauldron with DXMT backend
2. Run in both borderless windowed and exclusive fullscreen
3. Verify: no visible flickering or frame tearing
4. Verify: no performance regression (frame times should be equal or better)
5. Repeat with DXVK+MoltenVK backend
6. Repeat with D3DMetal backend (DX12 game)

## Priority Justification

This bug affects **every single game** on macOS Wine. It's the most visible quality issue in the entire stack. CrossOver 26 has this fix (their custom Wine fork includes it). Every Whisky fork user reports it. It's the #1 reason users perceive Wine-on-macOS as lower quality than Linux.

---

*Patch slot: `patches/cauldron/0003-winemac.drv-reduce-compositor-flicker.patch`*
