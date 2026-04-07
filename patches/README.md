# Cauldron Wine Patches

Patches applied on top of upstream Wine to create the Cauldron Wine fork.

## Patch Series

### 0001 — ntdll: Preserve private pages on VirtualProtect

**Bug:** [Wine #29384](https://bugs.winehq.org/show_bug.cgi?id=29384)

**Problem:** When a mod loader (SKSE, F4SE, etc.) writes code cave patches into
a PE's `.text` section and then calls `VirtualProtect` to restore
`PAGE_EXECUTE_READ`, Wine reloads file-backed pages from disk — wiping out all
in-memory patches. Windows preserves private copy-on-write pages across
protection changes.

**Root cause:** Wine's virtual memory manager doesn't track which file-backed
pages have been modified via copy-on-write. When protection is changed back to
read-only, macOS may discard the private page and reload from the backing file.
Additionally, on macOS, Wine uses `MAP_SHARED` for some read-only file mappings,
which prevents copy-on-write entirely.

**Fix:**
- Adds `VPROT_WRITTEN` flag to track pages modified via COW
- Traps writes on `VPROT_WRITECOPY` pages via `PROT_READ` mapping + fault handler
- Preserves `VPROT_WRITTEN` flag across `VirtualProtect` calls
- Forces `MAP_PRIVATE` for all file-backed mappings on macOS (enables COW)
- Based on wine-staging `ntdll-WRITECOPY` patchset, extended for macOS

**Affected software:** SKSE (Skyrim), F4SE (Fallout 4), OBSE (Oblivion),
ENBSeries, ReShade, ASI loaders, any DLL proxy mod that patches executable code.

## Adding New Patches

1. Create patch file: `patches/cauldron/NNNN-component-short-description.patch`
2. Follow standard git format-patch format (From/Subject/body/diff)
3. Number sequentially — patches are applied in sort order
4. Run `make wine-init-clean` to test patch application
5. CI automatically verifies patches apply against upstream Wine
