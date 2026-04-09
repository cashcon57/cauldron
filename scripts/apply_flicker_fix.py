#!/usr/bin/env python3
"""Apply the winemac.drv flicker fix to Wine 11.6 source."""
import sys
import os

wine_dir = sys.argv[1] if len(sys.argv) > 1 else "/tmp/cauldron-wine-x86/wine"
cocoa_path = os.path.join(wine_dir, "dlls/winemac.drv/cocoa_window.m")

if not os.path.exists(cocoa_path):
    print(f"Error: {cocoa_path} not found")
    sys.exit(1)

with open(cocoa_path) as f:
    content = f.read()

changes = 0

# Part 1: Wrap layer.contents update in CATransaction (in updateLayer)
old_update = """        if (image)
        {
            layer.position = surfaceRect.origin;
            layer.contents = (id)image;
            CFRelease(image);
            [window windowDidDrawContent];"""

new_update = """        if (image)
        {
            /* Cauldron: Atomic frame presentation via CATransaction.
             * Without this, the macOS compositor can read CALayer.contents
             * mid-update, causing visible flickering in every game. */
            [CATransaction begin];
            [CATransaction setDisableActions:YES];
            [CATransaction setAnimationDuration:0];

            layer.position = surfaceRect.origin;
            layer.contents = (id)image;

            [CATransaction commit];

            CFRelease(image);
            [window windowDidDrawContent];"""

if old_update in content:
    content = content.replace(old_update, new_update)
    changes += 1
    print("  Part 1: CATransaction wrapping in updateLayer")
else:
    print("  Part 1: SKIPPED (pattern not found)")

# Part 2: Suppress implicit CALayer animations in WineContentView init
old_init = """            [self setLayerContentsPlacement:NSViewLayerContentsPlacementTopLeft];
            [self setWantsLayer:YES];
            [self setLayerRetinaProperties:retina_on];
            [self setAutoresizesSubviews:NO];
        }"""

new_init = """            [self setLayerContentsPlacement:NSViewLayerContentsPlacementTopLeft];
            [self setWantsLayer:YES];
            [self setLayerRetinaProperties:retina_on];
            [self setAutoresizesSubviews:NO];

            /* Cauldron: Suppress implicit CALayer animations to prevent
             * Core Animation from interpolating between game frames. */
            self.layer.actions = @{
                @"contents": [NSNull null],
                @"bounds":   [NSNull null],
                @"position": [NSNull null],
            };
        }"""

if old_init in content:
    content = content.replace(old_init, new_init)
    changes += 1
    print("  Part 2: Implicit animation suppression in initWithFrame")
else:
    print("  Part 2: SKIPPED (pattern not found)")

# Part 3: Disable window animation behavior
# Find WineWindow creation and add animationBehavior after
old_window = """        window = [[[self alloc] initWithContentRect:window_frame"""
if old_window in content and "setAnimationBehavior" not in content:
    # Add animationBehavior right after window creation completes
    # Find the closing of the init block
    idx = content.index(old_window)
    # Search forward for the first ']' that closes the alloc chain, then find next newline
    search_from = idx + len(old_window)
    # Find "return window;" which is after all window setup
    return_idx = content.index("return window;", search_from)
    insert_point = content.rindex("\n", search_from, return_idx)
    content = content[:insert_point] + \
        "\n        /* Cauldron: Prevent window server frame interpolation. */" + \
        "\n        [window setAnimationBehavior:NSWindowAnimationBehaviorNone];" + \
        content[insert_point:]
    changes += 1
    print("  Part 3: Window animation behavior disabled")
else:
    if "setAnimationBehavior" in content:
        print("  Part 3: SKIPPED (already applied)")
    else:
        print("  Part 3: SKIPPED (pattern not found)")

with open(cocoa_path, "w") as f:
    f.write(content)

print(f"\nApplied {changes} changes to {cocoa_path}")
