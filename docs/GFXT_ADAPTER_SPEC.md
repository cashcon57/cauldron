# GFXT Adapter Specification for Cauldron

Specification for implementing Cauldron's D3DMetal integration layer.
Based on reverse engineering of CrossOver 24.x's libd3dshared.dylib.

## Executive Summary

Apple's D3DMetal.framework (GPTK) communicates with Wine through a C++ interface
hierarchy called "GFXT" (Graphics Framework Translation). The integration requires:

1. **PE stubs** (d3d11.dll, d3d12.dll, dxgi.dll, nvapi64.dll, nvngx.dll):
   - Thin Wine PE DLLs that export the D3D API surface
   - Dispatch to Unix side via `__wine_unix_call(handle, dispatch_id, &data)`
   - **dxgi.dll is the critical one** — it resolves 43 Win32 API pointers and passes them
     to the Unix side as the gWin32Dispatch table
   - Source: Apple's "D3DMetalDLLsBase-17.12/D3D4Mac/" project (found in debug symbols)

2. **Unix-side shared library** (`libd3dshared.dylib` or equivalent) that:
   - Is symlinked as `d3d11.so`, `d3d12.so`, `dxgi.so`, `nvapi64.so`, `nvngx.so`, `atidxx64.so`
   - Exports `__wine_unix_call_funcs` for Wine's unix call dispatch
   - Exports `GetMacDRVFunctions()` which resolves `macdrv_functions` from winemac.so
   - Implements 9 GFXT C++ interfaces that D3DMetal calls back into
   - dlopen's D3DMetal.framework and builds a dispatch table of its exported functions
   - Receives gWin32Dispatch (Win32 API pointers) from the PE side at init

3. The macOS Wine driver (winemac.so) must export `macdrv_functions` — a 24-entry
   function pointer table providing Metal device/view management and Win32 API wrappers.

## Two-Dispatch-Table Architecture

The system uses two function pointer tables that flow in opposite directions:

```
PE Side (Windows)                     Unix Side (macOS)
==================                    ==================

dxgi.dll resolves Win32 APIs ──────►  gWin32Dispatch (0x158 bytes, ~43 entries)
 (VirtualAlloc, CreateEventExA,         Stored in BSS, used by GFXT interfaces
  RegOpenKeyExA, MoveWindow, etc.)      as backend for Wine operations

gGFXTDispatch (0x160 bytes, ◄──────  D3DMetal exports dlsym'd at init
 44 entries)                            (GFXT_Initialize, D3D11CreateDevice,
 Filled by __wine_unix_call #0          D3D12CreateDevice, NVSDK_NGX_*, etc.)
 Used by PE exports to forward
 D3D calls to D3DMetal
```

Dispatch ID encoding: `(table_index << 16) | function_index`

**Critical insight**: D3DMetal requires ZERO Wine source patches. The entire integration
is done through these runtime interfaces.

## Architecture

```
Game.exe → d3d11.dll (PE stub, 114KB)
              │ __wine_unix_call
              ▼
         d3d11.so → libd3dshared.dylib (symlink)
              │
              ├─ dlopen("D3DMetal.framework/D3DMetal")
              │   └─ dlsym: GFXT_Initialize, D3D11CreateDevice, D3D12CreateDevice, ...
              │
              ├─ dlsym(RTLD_DEFAULT, "macdrv_functions")
              │   └─ 24-entry function pointer table from winemac.so
              │
              └─ GFXT_Initialize(WineOS*)
                  └─ D3DMetal queries Wine internals via 8 GFXT interfaces
```

## Initialization Sequence

### Step 1: Unix Call Entry
Wine loads d3d11.dll (PE), which calls `__wine_unix_call` into d3d11.so.
Since d3d11.so is a symlink to libd3dshared.dylib, `__wine_unix_call_funcs`
dispatches to `D3DRMDispatch_Init(void* dispatch_table)`.

### Step 2: D3DMetal Discovery
```c
// D3DRMDispatch_Init block_invoke:
handle = dlopen("/System/Library/Frameworks/D3DMetal.framework/D3DMetal", RTLD_NOW);
if (!handle)
    handle = dlopen("@rpath/D3DMetal.framework/D3DMetal", RTLD_NOW);
```

### Step 3: Dispatch Table Population
Resolves all D3DMetal exports and stores function pointers:
```c
gGFXTDispatch       = dlsym(handle, "GFXT_Initialize");
CreateDXGIFactory2  = dlsym(handle, "CreateDXGIFactory2");
D3D12CreateDevice   = dlsym(handle, "D3D12CreateDevice");
D3D11CreateDevice   = dlsym(handle, "D3D11CreateDevice");
D3D11CreateDeviceAndSwapChain = dlsym(handle, "D3D11CreateDeviceAndSwapChain");
D3D12SerializeRootSignature   = dlsym(handle, "D3D12SerializeRootSignature");
// ... plus D3D10CreateBlob, AmdDxExtCreate11, all NVSDK_NGX_* functions
// ... plus nvapi_QueryInterface, nvapi_Direct_GetMethod
```

### Step 4: Function Hooking
The real D3D functions are saved and replaced with `Shared_*` wrappers:
```c
gRealD3D12CreateDevice = D3D12CreateDevice;
D3D12CreateDevice = Shared_D3D12CreateDevice;   // calls InitSharedState first
gRealCreateDXGIFactory2 = CreateDXGIFactory2;
CreateDXGIFactory2 = Shared_CreateDXGIFactory2;
// etc.
```

### Step 5: WineOS Creation
```c
WineOS* os = new WineOS();  // creates all sub-interface objects
OSInterface = os;
dispatch_table.ThreadCallback = ThreadCallback;  // for D3DMetalWineThread
```

### Step 6: GFXT Initialization (lazy, on first D3D call)
```c
// InitSharedState():
GFXT_Initialize(OSInterface);  // passes WineOS* to D3DMetal
// D3DMetal now queries Wine for monitors, registry, paths, etc.
```

## GFXT Interface Specifications

### Common Pattern
All interfaces inherit from a base with:
- vtable[0]: `Version() const → int` (returns interface version)
- vtable[1]: `~destructor()`
- vtable[2]: `~destructor()` (deleting destructor)

### 1. GFXTOSInterface (WineOS) — Version 5

The root factory interface. D3DMetal receives this via `GFXT_Initialize()`.

```cpp
class GFXTOSInterface {
public:
    virtual int Version() const;  // returns 5
    virtual ~GFXTOSInterface();

    virtual GFXTMonitorInterface*    CreateMonitorInterface(const GFXTInterfaceVersion&);
    virtual GFXTRegistryInterface*   CreateRegistryInterface(const GFXTInterfaceVersion&);
    virtual GFXTEventInterface*      CreateEventInterface(const GFXTInterfaceVersion&);
    virtual GFXTSwapchainInterface*  CreateSwapchainInterface(const GFXTInterfaceVersion&, id<MTLDevice>);
    virtual GFXTAdapterInterface*    CreateAdapterInterface(const GFXTInterfaceVersion&);
    virtual GFXTPathInterface*       CreatePathInterface(const GFXTInterfaceVersion&);
    virtual GFXTAllocationInterface* CreateAllocationInterface(const GFXTInterfaceVersion&);
    virtual GFXTLibraryInterface*    CreateLibraryInterface(const GFXTInterfaceVersion&);
};
```

**Implementation notes:**
- WineOS pre-creates all sub-interfaces in constructor (stored at offsets +0x08 through +0x40)
- CreateEventInterface version check: if version.major == 2, returns WineEventCallbacks2_0
- CreateSwapchainInterface creates a new WineSwapchainCallbacks each time (takes MTLDevice*)
- Other Create* methods return pre-created singletons

### 2. GFXTMonitorInterface (WineMonitor) — Version 2

```cpp
class GFXTMonitorInterface {
public:
    virtual int Version() const;  // returns 2
    virtual ~GFXTMonitorInterface();

    // Populate monitor list using Win32 EnumDisplayMonitors
    static BOOL MonitorEnumProc(HMONITOR hmon, HDC hdc, LPRECT rect, WineMonitor* self);

    // Query cached monitor info by index
    virtual bool QueryMonitorInfo(uint32_t index, MonitorInfo& info);

    // Query display mode for a monitor
    virtual bool QueryDisplayMode(uint64_t displayID, const char16_t* deviceName,
                                  uint32_t modeIndex, DisplayModeInfo& info);

    // Query monitor description (adapter name, etc.)
    virtual bool QueryDescription(uint64_t displayID, MonitorDescription& desc);

    // Change display resolution
    virtual bool ChangeDisplayMode(uint64_t displayID, const char16_t* deviceName,
                                   const DisplayModeInfo& mode);
};

struct MonitorInfo {       // 0x20 bytes
    void*    hmonitor;     // +0x00
    uint32_t flags;        // +0x08
    int32_t  width;        // +0x0C
    int32_t  height;       // +0x10
    int32_t  physWidth;    // +0x14
    int32_t  physHeight;   // +0x18
};

struct DisplayModeInfo {   // 0x14 bytes
    uint64_t dimensions;   // +0x00  (width | height<<32)
    uint32_t format;       // +0x08
    uint32_t refreshNum;   // +0x0C  (refresh * 1000)
    uint32_t refreshDen;   // +0x10  (always 1000)
};
```

**Calls through macdrv_functions:** EnumDisplayMonitors[15], GetMonitorInfoA[16]

### 3. GFXTRegistryInterface (WineRegistry) — Version 2

```cpp
class GFXTRegistryInterface {
public:
    virtual int Version() const;  // returns 2
    virtual ~GFXTRegistryInterface();

    virtual void* OpenKey(RegistryMainKey root, const char* path);
    virtual void* CreateKey(RegistryMainKey root, const char* path, bool writeAccess, bool* created);
    virtual void  CloseKey(void* key);

    // SetValue overloads
    virtual void SetValue(void* key, const char* name, uint32_t value);
    virtual void SetValue(void* key, const char* name, uint64_t value);
    virtual void SetValue(void* key, const char* name, const std::string& value);
    virtual void SetValue(void* key, const char* name, const std::vector<uint8_t>& value);

    // GetValue overloads
    virtual bool GetValue(void* key, const char* name, uint32_t& value);
    virtual bool GetValue(void* key, const char* name, uint64_t& value);
    virtual bool GetValue(void* key, const char* name, std::string& value);
    virtual bool GetValue(void* key, const char* name, std::vector<uint8_t>& value);

    virtual void DeleteValue(void* key, const char* name, const char* subkey);
};
```

**Calls through macdrv_functions:** RegOpenKeyExA[12], RegCreateKeyExA[13], RegCloseKey[14],
RegQueryValueExA[10], RegSetValueExA[11]

### 4. GFXTEventInterface (WineEventCallbacks) — Version 1

```cpp
class GFXTEventInterface {
public:
    virtual int Version() const;  // returns 1 (or 2 for WineEventCallbacks2_0)
    virtual ~GFXTEventInterface();

    virtual void* CreateEvent(uint32_t access, bool manualReset);
    virtual void  SetEvent(void* handle);
    virtual void  ClearEvent(void* handle);
    virtual void  PulseEvent(void* handle);
    virtual void  CloseEvent(void* handle);
    virtual void* CreateSemaphore(uint32_t initial, uint32_t max);
    virtual void  SignalSemaphore(void* handle, uint32_t count);
    virtual void  CloseSemaphore(void* handle);
    virtual void* DuplicateEvent(void* handle);
    virtual void* DuplicateSemaphore(void* handle);
    // v1 only:
    virtual void  WaitSemaphore(void* handle, uint64_t timeout);
};
```

**Implementation:** Uses Wine NT event/semaphore APIs (NtCreateEvent, NtSetEvent, etc.)
The v2 interface (WineEventCallbacks2_0) adds `_DispatchFunctionInternal` which dispatches
operations to the D3DMetalWineThread for thread-safe execution.

**D3DMetalWineThread:** A dedicated thread named "D3DMetalWineThread" that processes
async event operations. Uses SyncHelper for wait/wake semantics.

### 5. GFXTSwapchainInterface (WineSwapchainCallbacks) — Version 1

**This is the most critical interface for rendering.**

```cpp
class GFXTSwapchainInterface {
public:
    virtual int Version() const;
    virtual ~GFXTSwapchainInterface();

    // Initialize swapchain for a Win32 HWND
    // Returns: true on success, fills resource_desc with backing dimensions and format
    virtual bool InitializeForHWND(HWND hwnd,
                                   const DXGI_SWAP_CHAIN_DESC1* desc,
                                   D3D12_RESOURCE_DESC& resource_desc);

    // Get the NSView for an HWND (from win_data offset +0x20)
    virtual void* GetViewForHWND(HWND hwnd);

    // Get next drawable from CAMetalLayer ([layer nextDrawable])
    virtual id<CAMetalDrawable> GetDrawableForHWND(HWND hwnd);

    // Resize the window
    virtual bool ResizeWindow(HWND hwnd, uint32_t width, uint32_t height);

    // Resize the Metal drawable (backing store)
    virtual bool ResizeBacking(HWND hwnd, uint32_t width, uint32_t height, DXGI_FORMAT format);

    // Enter/exit fullscreen
    virtual bool SetFullscreen(HWND hwnd, bool entering, bool changed,
                               uint32_t x, uint32_t y, uint32_t w, uint32_t h);

    // Present (no-op in current implementation — returns true)
    virtual bool Present(HWND hwnd);
};
```

**InitializeForHWND implementation:**
1. Calls `macdrv_functions.get_win_data(hwnd)` to get window info
2. Stores NSView reference from win_data
3. Creates CAMetalLayer via `macdrv_functions.view_get_metal_layer()`
4. Sets color space: `[layer setColorspace: CGColorSpaceCreateWithName(kCGColorSpaceSRGB)]`
5. Sets `[layer setWantsExtendedDynamicRangeContent: NO]`
6. Reads backing size from `[view frame]`
7. Maps CAMetalLayer pixel format to DXGI format:

**Pixel Format Mapping (CAMetalLayer → DXGI):**

| MTLPixelFormat value | Name                    | DXGI_FORMAT equivalent |
|---------------------|-------------------------|----------------------|
| 0x50 (80)           | MTLPixelFormatBGRA8Unorm | DXGI_FORMAT_B8G8R8A8_UNORM (0x57/87) |
| 0x51 (81)           | MTLPixelFormatBGRA8Unorm_sRGB | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB (0x5B/91) |
| 0x5A (90)           | MTLPixelFormatRGBA16Float | DXGI_FORMAT_R16G16B16A16_FLOAT (0x18/24) |
| 0x5E (94)           | MTLPixelFormatBGR10A2Unorm | DXGI_FORMAT_R16G16B16A16_FLOAT (0x18/24) |
| 0x73 (115)          | MTLPixelFormatRGBA8Unorm | DXGI_FORMAT_R8G8B8A8_UNORM (0x0A/10) |

**ResizeBacking format mapping (DXGI → MTLPixelFormat):**

| DXGI_FORMAT value | Name                          | MTLPixelFormat |
|-------------------|-------------------------------|---------------|
| 10 (0x0A)         | R8G8B8A8_UNORM                | 0x73 (RGBA8Unorm) |
| 24 (0x18)         | R16G16B16A16_FLOAT            | 0x5A (RGBA16Float) |
| 28 (0x1C)         | R8G8B8A8_UNORM (alt)          | 0x50 (BGRA8Unorm) |
| 29 (0x1D)         | R8G8B8A8_UNORM_SRGB           | 0x51 (BGRA8Unorm_sRGB) |
| 87 (0x57)         | B8G8R8A8_UNORM                | 0x50 (BGRA8Unorm) |
| 91 (0x5B)         | B8G8R8A8_UNORM_SRGB           | 0x51 (BGRA8Unorm_sRGB) |

### 6. GFXTAdapterInterface (WineAdapters) — Version 2

```cpp
class GFXTAdapterInterface {
public:
    virtual int Version() const;  // returns 2
    virtual ~GFXTAdapterInterface();

    // Fill array with GPU adapter LUIDs, returns count
    virtual size_t getAdapterLUIDs(LUID* luids, size_t maxCount);
};
```

**Implementation:** Enumerates GPU adapters and returns their LUIDs.

### 7. GFXTPathInterface (WinePaths) — Version 2

```cpp
class GFXTPathInterface {
public:
    virtual int Version() const;  // returns 2
    virtual ~GFXTPathInterface();

    virtual void windowsToUnixPath(const char16_t* winPath, char* unixPath, size_t* len);
    virtual void unixToWindowsPath(const char* unixPath, char16_t* winPath, size_t* len);
    virtual void windowsSystemDirectoryPath(char16_t* path, size_t* len);
    virtual void getExecutablePath(char* path, uint32_t maxLen);
    virtual void getModulePath(void* module, char* path, uint32_t maxLen);
};
```

**Implementation:** Wraps Wine's path conversion APIs (wine_nt_to_unix_file_name, etc.)

### 8. GFXTAllocationInterface (WineAllocations) — Version 2

```cpp
class GFXTAllocationInterface {
public:
    virtual int Version() const;  // returns 2
    virtual ~GFXTAllocationInterface();

    // VirtualAlloc(NULL, size, MEM_COMMIT|MEM_RESERVE, PAGE_READWRITE)
    virtual void* allocateBytesFromNewPages(size_t size);

    // VirtualFree(ptr, size, MEM_RELEASE)
    virtual void freeBytesFromAllocatedPages(void* ptr, size_t size);

    virtual void* malloc(size_t size);
    virtual void  free(void* ptr);

    // VirtualProtect to PAGE_EXECUTE_READ
    virtual void makeExecutable(void* ptr, size_t size);

    // Special: for "dxgi" image, allocates from reserved contiguous range
    // Otherwise falls back to allocateBytesFromNewPages
    virtual void* allocateBytesFromImage(const char* imageName, size_t size);
};
```

**Note:** `allocateBytesFromImage("dxgi")` uses a special pre-reserved memory range
with an unfair lock for thread safety. This ensures DXGI allocations are contiguous.

### 9. GFXTLibraryInterface (WineLibraries) — Version 2

```cpp
class GFXTLibraryInterface {
public:
    virtual int Version() const;  // returns 2
    virtual ~GFXTLibraryInterface();

    virtual void* loadLibrary(const char* name);                    // LoadLibraryA
    virtual void* getModuleHandle(const char* name);                // GetModuleHandleA
    virtual void* getProcAddress(void* module, const char* name);   // GetProcAddress
    virtual void  freeLibrary(void* module);                        // FreeLibrary
    virtual void* loadLibraryFromSystemDirectory(const char* name); // LoadLibrary from system32
};
```

## Rosetta 2 Integration

```c
// Check if running under Rosetta translation
bool _supports_non_native_code_regions() {
    int translated = 0;
    size_t size = 4;
    sysctlbyname("sysctl.proc_translated", &translated, &size, NULL, 0);
    if (translated) {
        void* lib = dlopen("/usr/lib/libRosetta.dylib", RTLD_NOLOAD);
        auto getVersion = dlsym(lib, "rosetta_get_runtime_version");
        long version = getVersion();
        // Check version >= threshold
        NonNativeRegionSupported = true;
    }
}

// Register JIT-compiled shader code with Rosetta
void _register_non_native_code_region(void* addr, size_t size) {
    if (NonNativeRegionSupported) {
        syscall(0x7000009, addr, size);
    }
}
```

## SyncHelper

Two implementations based on OS version:

**SyncHelper_API** (macOS 14+): Uses `os_sync_wait_on_address` / `os_sync_wake_by_address_*`
**SyncHelper_Old** (fallback): Spin-wait loop:
1. Spin 64 iterations checking value
2. If elapsed < 4000ns: busy spin
3. If elapsed < ~64000ns: `sched_yield()`
4. If elapsed > ~64000ns: `__libcpp_atomic_wait` (futex-like)

## DXMT vs D3DMetal Comparison

| Aspect | D3DMetal (GPTK) | DXMT (CodeWeavers) |
|--------|----------------|-------------------|
| Integration | Via libd3dshared GFXT interfaces | Direct link to winemac.so |
| API coverage | D3D11 + D3D12 + DXGI | D3D10 + D3D11 + DXGI only |
| Shader compilation | libmetalirconverter + dxccontainer | Built-in LLVM (DXBC→AIR) |
| Binary size | 5.3MB framework + 96KB glue | 24MB single .so |
| macdrv usage | Via macdrv_functions[4-8] (Metal view mgmt) | Direct dlsym of same 3 functions |
| Wine patches needed | ZERO | ZERO (links to stock winemac.so) |
| Unix call dispatch | libd3dshared exports `__wine_unix_call_funcs` | winemetal.so exports `__wine_unix_call_funcs` |

**Key difference:** DXMT uses only 3 macdrv functions (`view_create_metal_view`,
`view_get_metal_layer`, `view_release_metal_view`) resolved via strings in the binary.
D3DMetal uses all 24 through the macdrv_functions table pointer.

## Implementation Priority for Cauldron

### Critical (must implement):
1. **macdrv_functions table** in winemac.so — all 24 entries
2. **GFXT interfaces** — WineOS, WineSwapchainCallbacks, WineEventCallbacks
3. **D3DMetal dispatch table** — dlopen + dlsym for all required exports
4. **Rosetta code region registration** — for shader JIT

### Important (needed for correctness):
5. **WineMonitor** — display enumeration and mode switching
6. **WineRegistry** — D3DMetal reads/writes config from Wine registry
7. **WineAllocations** — memory management with special DXGI range
8. **SyncHelper** — wait/wake primitives

### Nice to have:
9. **WineLibraries** — DLL loading (mostly for NVAPI stubs)
10. **WinePaths** — path conversion (used for shader cache)
11. **WineAdapters** — GPU enumeration (single GPU on most Macs)

## File Layout

Cauldron's GPTK integration should produce:
```
lib64/apple_gptk/
├── external/
│   ├── D3DMetal.framework/    (from system or bundled)
│   └── libd3dshared.dylib     (Cauldron's GFXT adapter - THIS IS WHAT WE BUILD)
└── wine/
    ├── x86_64-unix/
    │   ├── d3d11.so  → ../../external/libd3dshared.dylib
    │   ├── d3d12.so  → ../../external/libd3dshared.dylib
    │   ├── dxgi.so   → ../../external/libd3dshared.dylib
    │   ├── nvapi64.so → ../../external/libd3dshared.dylib
    │   └── nvngx.so  → ../../external/libd3dshared.dylib
    └── x86_64-windows/
        ├── d3d11.dll   (PE stub — calls __wine_unix_call)
        ├── d3d12.dll
        ├── dxgi.dll
        ├── nvapi64.dll
        └── nvngx.dll
```
