# CrossOver D3DMetal Integration Architecture

Reverse-engineered via Ghidra headless analysis of CrossOver 24.x binaries.
All analysis performed for interoperability/clean-room reimplementation purposes.

## Binary Inventory

| Binary | Size | Role |
|--------|------|------|
| `lib/dxmt/x86_64-unix/winemetal.so` | 24MB | DXMT: DXBC->AIR shader compiler + Metal rendering backend |
| `lib/dxmt/x86_64-windows/winemetal.dll` | - | Windows-side DXMT stub (PE) |
| `lib/dxmt/x86_64-windows/d3d11.dll` | - | DXMT D3D11 implementation (replaces Wine's) |
| `lib/dxmt/x86_64-windows/dxgi.dll` | - | DXMT DXGI implementation |
| `lib/wine/x86_64-unix/winemac.so` | 467KB | Wine macOS driver (Cocoa, display, Metal view mgmt) |
| `lib64/apple_gptk/external/D3DMetal.framework` | 5.3MB | Apple's Game Porting Toolkit D3D->Metal translation |
| `lib64/apple_gptk/external/libd3dshared.dylib` | 96KB | **THE GLUE** - bridges Wine to D3DMetal via GFXT interfaces |

## Architecture Overview

```
Windows Game (D3D11/D3D12 calls)
        |
        v
  +-----------------+       +-----------------+
  | DXMT layer      |       | Apple GPTK      |
  | winemetal.dll   |       | D3DMetal.fwk    |
  | d3d11.dll       |       | (D3D->Metal)    |
  | dxgi.dll        |       |                 |
  +--------+--------+       +--------+--------+
           |                          |
           v                          v
  +-----------------+       +-----------------+
  | winemetal.so    |       | libd3dshared    |
  | (Unix side)     |<----->| (GFXT glue)     |
  | DXBC->AIR+LLVM  |       | Wine adapters   |
  +--------+--------+       +--------+--------+
           |                          |
           v                          v
  +-----------------+       +-----------------+
  | winemac.so      |<------| macdrv_functions|
  | (Wine macOS drv)|       | (function table)|
  +--------+--------+       +-----------------+
           |
           v
  Metal.framework / AppKit / CoreGraphics
```

## Key Discovery: The GFXT Interface System

D3DMetal.framework communicates with Wine through a C++ interface hierarchy defined in
`libd3dshared.dylib`. The framework calls `GFXT_Initialize()` and receives an OS interface
object that provides access to Wine's internals.

### Interface Hierarchy

```
GFXTOSInterface (v5)         -- WineOS
  ├── GFXTMonitorInterface (v2)    -- WineMonitor
  ├── GFXTRegistryInterface (v2)   -- WineRegistry  
  ├── GFXTEventInterface (v1/v2)   -- WineEventCallbacks / WineEventCallbacks2_0
  ├── GFXTSwapchainInterface       -- WineSwapchainCallbacks
  ├── GFXTAdapterInterface         -- WineAdapters
  ├── GFXTPathInterface            -- WinePaths
  ├── GFXTAllocationInterface (v2) -- WineAllocations
  └── GFXTLibraryInterface (v2)    -- WineLibraries
```

### Interface Details (from Ghidra decompilation)

#### GFXTOSInterface / WineOS
- Factory class that creates all sub-interfaces
- `CreateMonitorInterface()`, `CreateRegistryInterface()`, etc.
- `CreateSwapchainInterface(version, MTLDevice*)` -- passes Metal device to swapchain
- `CreateEventInterface(version)` -- version 2 returns WineEventCallbacks2_0

#### GFXTMonitorInterface / WineMonitor
- `MonitorEnumProc(hMonitor, hDC, lpRect, this)` -- MONITORINFO struct, 0x68 bytes
- `QueryMonitorInfo(index, MonitorInfo&)` -- returns from cached vector
- `QueryDisplayMode(displayID, deviceName, index, DisplayModeInfo&)` -- wraps DEVMODE
- `QueryDescription(displayID, MonitorDescription&)` -- display adapter info
- `ChangeDisplayMode(displayID, deviceName, DisplayModeInfo&)` -- mode switching

#### GFXTRegistryInterface / WineRegistry
- `OpenKey(RegistryMainKey, path)` -- opens Wine registry key
- `CreateKey(RegistryMainKey, path, ...)` -- creates registry key
- `SetValue(key, name, uint32/uint64/string/binary)` -- 4 overloads
- `GetValue(key, name, uint32/uint64/string/binary)` -- 4 overloads
- `DeleteValue(key, name, ...)` -- removes registry value

#### GFXTEventInterface / WineEventCallbacks
- `CreateEvent(access, manual_reset)` -- returns Wine event handle
- `SetEvent(handle)`, `ClearEvent(handle)`, `PulseEvent(handle)`, `CloseEvent(handle)`
- `CreateSemaphore(initial, max)`, `SignalSemaphore(handle, count)`, `CloseSemaphore(handle)`
- `DuplicateEvent(handle)`, `DuplicateSemaphore(handle)`
- `_DispatchFunctionInternal(func, async, data, flags)` -- dispatches to D3DMetalWineThread
- v2.0 adds `WaitSemaphore(handle, timeout)` via direct Wine calls

#### GFXTSwapchainInterface / WineSwapchainCallbacks  
- `InitializeForHWND(hwnd, DXGI_SWAP_CHAIN_DESC1*, D3D12_RESOURCE_DESC&)`:
  - Gets macdrv_functions via GetMacDRVFunctions()
  - Calls macdrv to get window info from HWND
  - Gets NSView and creates CAMetalLayer
  - Sets color space to sRGB
  - Maps pixel formats: 0x50->0x57, 0x51->0x5b, 0x5a/0x5e->0x18, 0x73->10
  - Returns backing dimensions in D3D12_RESOURCE_DESC
- `GetViewForHWND(hwnd)` -- returns NSView from window info
- `GetDrawableForHWND(hwnd)` -- calls [metalLayer nextDrawable]  
- `ResizeWindow(hwnd, width, height)` -- repositions/resizes via macdrv
- `ResizeBacking(hwnd, width, height, format)` -- calls [metalLayer setDrawableSize:]
  - Maps DXGI_FORMAT to MTLPixelFormat
- `SetFullscreen(hwnd, entering, changed, x, y, w, h)` -- fullscreen transitions
- `Present(hwnd)` -- returns 1 (no-op, presentation via CAMetalDrawable)

#### GFXTAdapterInterface / WineAdapters
- `getAdapterLUIDs(luid_array, count)` -- enumerates GPU adapters via LUID

#### GFXTPathInterface / WinePaths
- `windowsToUnixPath(wchar*, char*, size_t*)` -- NT path to Unix conversion
- `unixToWindowsPath(char*, wchar*, size_t*)` -- Unix to NT path conversion
- `windowsSystemDirectoryPath(wchar*, size_t*)` -- system32 path
- `getExecutablePath(char*, uint)` -- current process path
- `getModulePath(module, char*, uint)` -- module file path

#### GFXTAllocationInterface / WineAllocations
- `allocateBytesFromNewPages(size)` -- VirtualAlloc(MEM_COMMIT|MEM_RESERVE, PAGE_READWRITE)
- `freeBytesFromAllocatedPages(ptr, size)` -- VirtualFree(MEM_RELEASE)
- `malloc(size)` / `free(ptr)` -- direct libc
- `makeExecutable(ptr, size)` -- VirtualProtect to executable
- `allocateBytesFromImage(name, size)` -- special: for "dxgi" image, allocates from reserved range

#### GFXTLibraryInterface / WineLibraries
- `loadLibrary(name)` -- LoadLibraryA
- `getModuleHandle(name)` -- GetModuleHandleA
- `getProcAddress(module, name)` -- GetProcAddress
- `freeLibrary(module)` -- FreeLibrary
- `loadLibraryFromSystemDirectory(name)` -- loads from system32

## Initialization Flow

1. **D3DRMDispatch_Init()** is the entry point:
   - Tries `dlopen("/System/Library/Frameworks/D3DMetal.framework/D3DMetal")`
   - Falls back to `dlopen("@rpath/D3DMetal.framework/D3DMetal")`
   - Resolves via `dlsym`: `GFXT_Initialize`, `CreateDXGIFactory2`, `D3D12CreateDevice`,
     `D3D11CreateDevice`, `D3D11CreateDeviceAndSwapChain`, `D3DCompile`, plus NVAPI/NGX stubs
   - **Hooks the dispatch table**: replaces real D3D funcs with `Shared_*` wrappers that
     call `InitSharedState()` first, then forward to D3DMetal
   - Creates `WineOS` instance as the OS interface
   - Sets `ThreadCallback` for D3DMetalWineThread (processes async events)

2. **InitSharedState()** (called lazily on first D3D call):
   - Calls `GFXT_Initialize(OSInterface)` -- passes WineOS to D3DMetal
   - This is where D3DMetal queries Wine for monitors, registry, paths, etc.

3. **GetMacDRVFunctions()** -- resolved via `dlsym(RTLD_DEFAULT, "macdrv_functions")`:
   - Returns a function pointer table from winemac.so
   - Used by WineSwapchainCallbacks to manage windows/views/Metal layers

## Rosetta 2 Integration

`libd3dshared.dylib` includes Rosetta detection and support:
- `_supports_non_native_code_regions()` checks `sysctl.proc_translated`
- If running under Rosetta, loads `/usr/lib/libRosetta.dylib`
- Calls `rosetta_get_runtime_version()` to check version compatibility
- `_register_non_native_code_region(addr, size)` uses `syscall(0x7000009)` to register
  JIT-compiled code regions with Rosetta (for shader compilation output)

## SyncHelper System

Two sync implementations depending on OS version:
- **SyncHelper_API** (modern): Uses `os_sync_wait_on_address` / `os_sync_wake_by_address_*`
- **SyncHelper_Old** (fallback): Spin-wait with backoff (64 spins, then yield, then futex-like)

## DXMT (winemetal.so) Architecture

The 24MB binary contains:
- **LLVM compiler infrastructure**: Full LLVM pass pipeline for shader compilation
- **DXBC->AIR converter**: `dxmt::dxbc::convertDXBC()` translates D3D shader bytecode
  - Handles VS, PS, GS, HS, DS, CS, MS shader types
  - `dxmt::air::*` namespace builds Metal AIR (Apple Intermediate Representation)
  - Uses argument buffers, function signatures, tessellation support
- **Metal API integration**: Direct use of MTL* ObjC classes for:
  - Render pipelines, compute pipelines, depth/stencil
  - Texture descriptors, sampler descriptors
  - Binary archives (shader caching)
  - MetalFX (FSR/DLSS equivalent: spatial + temporal scalers)
  - Shared texture handles (for cross-process)
- **Links to winemac.so** at load time via `@rpath/winemac.so`
- **Links to ntdll.so** for Wine kernel calls
- `_WMTSetMetalShaderCachePath()` -- configures shader cache location

## D3DMetal.framework (Apple GPTK) Architecture

Exports the D3D API surface that games call:
- `D3D11CreateDevice`, `D3D11CreateDeviceAndSwapChain`
- `D3D12CreateDevice`, `CreateDXGIFactory`, `CreateDXGIFactory2`
- `D3DCompile`, `D3DCompileFromFile`
- `D3D12SerializeRootSignature`, etc.
- `GFXT_Initialize` -- the bridge entry point
- `AmdDxExtCreate11` -- AMD extension stubs
- Full NVSDK_NGX support for DLSS (D3D11 + D3D12)
- `DXGIDeclareAdapterRemovalSupport`

Links to `libmetalirconverter.dylib` and `libdxccontainer.dylib` for shader compilation.

## winemac.so Key Interfaces

Exports only 3 symbols:
- `__wine_unix_call_funcs` -- Wine unix call dispatch
- `__wine_unix_call_wow64_funcs` -- WoW64 dispatch
- `macdrv_functions` -- **THE function table** that libd3dshared resolves at runtime

Key internal functions (from Ghidra):
- `WineContentView::newMetalViewWithDevice_()` -- creates WineMetalView as subview
- `WineContentView::setLayerRetinaProperties_()` -- configures CALayer scaling (nearest vs linear)
- `WineContentView::makeBackingLayer()` -- creates the backing CALayer
- `resetSurfaceIfBackingSizeChanged()` -- handles resize
- `updateLayer()` -- layer update callback
- `setSurfaceRect:()` -- positions the rendering surface
- `clearDrawable()` / `clearDrawableLeavingSurfaceOnScreen()` -- Metal drawable management

## CrossOver Main Binary

The CrossOver app (2.4MB universal) orchestrates bottle management and Wine prefix setup.
Key functions relate to bottle configuration, not D3D integration directly.

## Implications for Cauldron

### What we need to implement:
1. **GFXT interface adapters** -- Cauldron needs its own `libd3dshared`-equivalent that:
   - Implements all 8 GFXT interfaces with correct vtable layout
   - Bridges to our Wine fork's registry, events, paths, allocations
   - Provides `macdrv_functions` table access

2. **Swapchain management** -- The swapchain interface is the most complex:
   - Must create CAMetalLayer on NSView from HWND
   - Handle pixel format mapping (DXGI_FORMAT -> MTLPixelFormat)
   - Manage drawable lifecycle and present

3. **DXMT vs D3DMetal choice**:
   - DXMT (winemetal.so) is CodeWeavers' own D3D11 implementation, NOT Apple's
   - D3DMetal.framework is Apple's GPTK, handles both D3D11 and D3D12
   - CrossOver ships BOTH and can switch between them
   - For Cauldron: we could use either DXVK (Vulkan-based) or target GPTK compatibility

4. **Rosetta code region registration** -- Important for shader JIT on Apple Silicon

### Interface versions to match:
- GFXTOSInterface: v5
- GFXTMonitorInterface: v2
- GFXTRegistryInterface: v2
- GFXTEventInterface: v1 (v2 available)
- GFXTAllocationInterface: v2
- GFXTLibraryInterface: v2

## Raw Data Locations

All Ghidra analysis output saved to `/tmp/ghidra_crossover_analysis/`:
- `d3dshared_decompiled.c` (4078 lines) -- full libd3dshared decompilation
- `winemac_decompiled.c` (47148 lines) -- full winemac.so decompilation
- `d3dmetal_decompiled.c` (120160 lines) -- full D3DMetal decompilation
- `crossover_decompiled.c` (87320 lines) -- full CrossOver binary decompilation
- `winemetal_symbols.txt` (15090 symbols) -- winemetal.so symbol table
