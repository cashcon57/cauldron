import Foundation

@MainActor
final class CauldronBridge: Sendable {
    static let shared = CauldronBridge()

    private nonisolated(unsafe) var managerPtr: UnsafeMutableRawPointer?

    init() {
        let appSupportDir = Self.appSupportDirectory()
        managerPtr = appSupportDir.withCString { cStr in
            cauldron_init(cStr)
        }
        if managerPtr == nil {
            print("[CauldronBridge] Warning: cauldron_init returned nil")
        }
    }

    deinit {
        if let ptr = managerPtr {
            cauldron_free(ptr)
        }
    }

    // MARK: - Public API

    func createBottle(name: String, wineVersion: String) -> Bottle? {
        guard let ptr = managerPtr else { return nil }

        let jsonPtr = name.withCString { nameCStr in
            wineVersion.withCString { versionCStr in
                cauldron_create_bottle(ptr, nameCStr, versionCStr)
            }
        }

        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    func listBottles() -> [Bottle] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr = cauldron_list_bottles(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    func deleteBottle(id: String) -> Bool {
        guard let ptr = managerPtr else { return false }
        let result = id.withCString { idCStr in
            cauldron_delete_bottle(ptr, idCStr)
        }
        return result == 0
    }

    // MARK: - Game Library

    func listGames() -> [GameRecord] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr = cauldron_list_games(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    func queryGame(appId: UInt32) -> GameRecord? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = cauldron_query_game(ptr, appId)
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Sync

    func getSyncStatus() -> SyncStatus? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = cauldron_get_sync_status(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Sync Actions

    nonisolated func runSync() -> SyncStatus? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = cauldron_run_sync(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Launch

    /// Launch a game with the full effective settings applied.
    /// `settings` contains the merged global profile + per-game overrides.
    nonisolated func launchExe(bottleId: String, exePath: String, settings: LaunchSettings) -> Bool {
        guard let ptr = managerPtr else { return false }
        guard let jsonData = try? JSONEncoder().encode(settings),
              let jsonString = String(data: jsonData, encoding: .utf8) else { return false }

        let result = bottleId.withCString { bidCStr in
            exePath.withCString { exeCStr in
                jsonString.withCString { settingsCStr in
                    cauldron_launch_exe(ptr, bidCStr, exeCStr, settingsCStr)
                }
            }
        }
        return result == 0
    }

    /// Convenience launcher that builds settings from AppSettings + optional per-game overrides.
    nonisolated func launchExe(bottleId: String, exePath: String, backend: String) -> Bool {
        // Build default settings from the backend string (backward compat)
        let settings = LaunchSettings(
            backend: backend,
            msync: true,
            esync: true,
            rosettaX87: false,
            asyncShaders: true,
            metalfxSpatial: false,
            metalHud: false,
            dxrEnabled: false,
            mvkArgumentBuffers: false,
            fsr: false,
            largeAddressAware: false,
            logLevel: "normal",
            highResolution: false,
            frameRateLimit: 0,
            heapZeroMemory: false
        )
        return launchExe(bottleId: bottleId, exePath: exePath, settings: settings)
    }

    /// Kill all Wine processes in a bottle (for restart / backend change).
    nonisolated func killBottle(bottleId: String) -> Int {
        guard let ptr = managerPtr else { return -1 }
        let result = bottleId.withCString { cauldron_kill_bottle(ptr, $0) }
        return Int(result)
    }

    /// Check if a bottle has any running Wine processes.
    nonisolated func isBottleRunning(bottleId: String) -> Bool {
        guard let ptr = managerPtr else { return false }
        let result = bottleId.withCString { cauldron_is_bottle_running(ptr, $0) }
        return result == 1
    }

    /// Settings struct passed as JSON to the Rust launch function.
    struct LaunchSettings: Codable {
        let backend: String
        let msync: Bool
        let esync: Bool
        let rosettaX87: Bool
        let asyncShaders: Bool
        let metalfxSpatial: Bool
        let metalHud: Bool
        let dxrEnabled: Bool
        let mvkArgumentBuffers: Bool
        let fsr: Bool
        let largeAddressAware: Bool
        let logLevel: String
        let highResolution: Bool
        let frameRateLimit: Int
        let heapZeroMemory: Bool

        /// Build from AppSettings global profile + optional per-game overrides.
        @MainActor
        static func from(appSettings: AppSettings, perGame: PerGameSettings?, backend: GraphicsBackend? = nil) -> LaunchSettings {
            let s = appSettings
            return LaunchSettings(
                backend: (perGame?.graphicsBackend ?? backend ?? s.defaultGraphicsBackend).rawValue,
                msync: perGame?.msyncEnabled ?? true,
                esync: perGame?.esyncEnabled ?? true,
                rosettaX87: perGame?.rosettaX87Enabled ?? s.rosettaX87Enabled,
                asyncShaders: perGame?.asyncShaderCompilation ?? s.asyncShaderCompilation,
                metalfxSpatial: perGame?.metalFXSpatialUpscaling ?? s.metalFXSpatialUpscaling,
                metalHud: perGame?.metalPerformanceHUD ?? s.metalPerformanceHUD,
                dxrEnabled: perGame?.dxrRayTracing ?? s.dxrRayTracing,
                mvkArgumentBuffers: perGame?.moltenVKArgumentBuffers ?? s.moltenVKArgumentBuffers,
                fsr: perGame?.fsrEnabled ?? s.fsrEnabled,
                largeAddressAware: perGame?.largeAddressAware ?? false,
                logLevel: s.logLevel.rawValue,
                highResolution: perGame?.highResolutionMode ?? s.highResolutionMode,
                frameRateLimit: perGame?.frameRateLimit ?? s.frameRateLimit,
                heapZeroMemory: perGame?.heapZeroMemory ?? s.heapZeroMemory
            )
        }
    }

    // MARK: - Wine Versions

    func getWineVersions() -> [WineVersionInfo] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr = cauldron_get_wine_versions(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    /// Download and install a Wine version. This is a blocking call.
    /// Returns a WineDownloadResult with the path on success or an error message.
    nonisolated func downloadWine(version: String) -> WineDownloadResult? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = version.withCString { versionCStr in
            cauldron_download_wine(ptr, versionCStr)
        }
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    /// Get all installed Wine versions with their paths.
    func getInstalledWine() -> [WineVersionInfo] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr = cauldron_get_installed_wine(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    // MARK: - Patch Management

    func getProtonCommits(filter: String? = nil, limit: UInt32 = 100) -> [ProtonCommitEntry] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr: UnsafeMutablePointer<CChar>?
        if let filter = filter {
            jsonPtr = filter.withCString { filterCStr in
                cauldron_get_proton_commits(ptr, filterCStr, limit)
            }
        } else {
            jsonPtr = cauldron_get_proton_commits(ptr, nil, limit)
        }
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    nonisolated func applyPatch(hash: String) -> PatchActionResult? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = hash.withCString { hashCStr in
            cauldron_apply_patch(ptr, hashCStr)
        }
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    nonisolated func skipPatch(hash: String) -> PatchActionResult? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = hash.withCString { hashCStr in
            cauldron_skip_patch(ptr, hashCStr)
        }
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    nonisolated func reversePatch(hash: String) -> PatchActionResult? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = hash.withCString { hashCStr in
            cauldron_reverse_patch(ptr, hashCStr)
        }
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Dependencies

    struct DependencyInfo: Codable, Identifiable {
        let id: String
        let name: String
        let description: String
        let category: String
        let recommended: Bool
    }

    struct DependencyResult: Codable {
        let dependencyId: String
        let success: Bool
        let error: String?
        let method: String
    }

    func listDependencies() -> [DependencyInfo] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr = cauldron_list_dependencies(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    nonisolated func installDependency(bottleId: String, dependencyId: String) -> DependencyResult? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = bottleId.withCString { bidCStr in
            dependencyId.withCString { depCStr in
                cauldron_install_dependency(ptr, bidCStr, depCStr)
            }
        }
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - D3DMetal / GPTK

    struct D3DMetalInfo: Codable {
        let source: String   // "crossover", "gptk", "custom", "imported", "none"
        let label: String
        let path: String
        let imported: Bool
    }

    struct ImportResult: Codable {
        let success: Bool
        let error: String?
        let path: String?
        let source: String?
    }

    func detectD3DMetal() -> D3DMetalInfo? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = cauldron_detect_d3dmetal(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    nonisolated func importD3DMetal(customPath: String? = nil) -> ImportResult? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr: UnsafeMutablePointer<CChar>?
        if let path = customPath {
            jsonPtr = path.withCString { cauldron_import_d3dmetal(ptr, $0) }
        } else {
            jsonPtr = cauldron_import_d3dmetal(ptr, nil)
        }
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Patch Analysis

    nonisolated func analyzePatches() -> [PatchAnalysis] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr = cauldron_analyze_patches(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    nonisolated func verifyBuild() -> PatchActionResult? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = cauldron_verify_build(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Bottle Scanning

    func scanBottleGames(bottleId: String) -> [GameRecord] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr = bottleId.withCString { idCStr in
            cauldron_scan_bottle_games(ptr, idCStr)
        }
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    // MARK: - Bottle Discovery & Import

    func discoverBottles() -> [DiscoveredBottle] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr = cauldron_discover_bottles(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    /// Import a discovered bottle by symlinking it into Cauldron's managed directory.
    /// Returns the imported Bottle on success, nil on failure.
    func importBottle(sourcePath: String, name: String) -> Bottle? {
        // Debug to file since GUI apps buffer stdout
        let log = { (msg: String) in
            let path = "/tmp/cauldron_bridge_debug.log"
            let existing = (try? String(contentsOfFile: path, encoding: .utf8)) ?? ""
            try? (existing + msg + "\n").write(toFile: path, atomically: true, encoding: .utf8)
        }
        log("importBottle called: sourcePath=\(sourcePath) name=\(name)")

        guard let ptr = managerPtr else {
            log("managerPtr is nil!")
            return nil
        }
        log("managerPtr OK")
        let jsonPtr = sourcePath.withCString { srcCStr in
            name.withCString { nameCStr in
                cauldron_import_bottle(ptr, srcCStr, nameCStr)
            }
        }
        guard let resultString = consumeCString(jsonPtr) else {
            print("[CauldronBridge] importBottle: got nil from FFI")
            return nil
        }
        log("importBottle JSON: \(resultString.prefix(500))")

        let bottle: Bottle? = decodeJSON(resultString)
        if bottle == nil {
            log("DECODE FAILED for Bottle type")
        } else {
            log("DECODE OK: \(bottle!.name)")
        }
        return bottle
    }

    // MARK: - RosettaX87

    struct RosettaX87Status: Codable {
        let available: Bool
        let path: String
        let label: String
    }

    func detectRosettaX87() -> RosettaX87Status? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = cauldron_detect_rosettax87(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Game Binary Patches

    struct GamePatchResult: Codable {
        let gameTitle: String?
        let exePath: String?
        let patchesApplied: Int?
        let patchesAvailable: Int?
        let alreadyPatched: Bool?
        let canRestore: Bool?
        let error: String?
    }

    struct GamePatchSet: Codable, Identifiable {
        var id: String { title }
        let title: String
        let steamAppId: Int?
        let exeName: String
        let category: String?
    }

    func scanGamePatches(bottleId: String) -> [GamePatchResult] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr = bottleId.withCString { cauldron_scan_game_patches(ptr, $0) }
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    nonisolated func applyGamePatch(exePath: String) -> GamePatchResult? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = exePath.withCString { cauldron_apply_game_patch(ptr, $0) }
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    nonisolated func restoreGameExe(exePath: String) -> Bool {
        guard let ptr = managerPtr else { return false }
        let jsonPtr = exePath.withCString { cauldron_restore_game_exe(ptr, $0) }
        guard let resultString = consumeCString(jsonPtr) else { return false }
        struct Result: Codable { let success: Bool }
        let result: Result? = decodeJSON(resultString)
        return result?.success ?? false
    }

    func listKnownGamePatches() -> [GamePatchSet] {
        guard let ptr = managerPtr else { return [] }
        let jsonPtr = cauldron_list_known_game_patches(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return [] }
        return decodeJSON(resultString) ?? []
    }

    // MARK: - Game Profiles

    struct SeedResult: Codable {
        let success: Bool
        let profilesSeeded: Int?
        let totalProfiles: Int?
    }

    func seedGameProfiles() -> SeedResult? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = cauldron_seed_game_profiles(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Game Recommendations

    struct GameRecommendation: Codable {
        let found: Bool
        let backend: String?
        let notes: String?
        let rosettaX87: Bool?
        let asyncShader: Bool?
        let metalfxUpscaling: Bool?
        let dxrRayTracing: Bool?
        let fsrEnabled: Bool?
        let windowsVersion: String?
        let launchArgs: String?
        let autoApplyPatches: Bool?
    }

    func getGameRecommendation(appId: UInt32) -> GameRecommendation? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = cauldron_get_game_recommendation(ptr, appId)
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Runtime Downloads

    struct RuntimeDownloadResults: Codable {
        let success: Bool
        let results: [RuntimeDownloadEntry]?
    }

    struct RuntimeDownloadEntry: Codable {
        let component: String
        let version: String
        let status: String
        let error: String?
    }

    /// Download all graphics runtimes (DXVK, DXMT, MoltenVK, VKD3D-Proton).
    /// Blocking call — run from a background thread.
    nonisolated func downloadAllRuntimes() -> RuntimeDownloadResults? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = cauldron_download_all_runtimes(ptr)
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Backend Switching

    struct SwitchBackendResult: Codable {
        let success: Bool
        let message: String?
        let error: String?
    }

    /// Switch a bottle's graphics backend. Downloads runtime if needed,
    /// swaps DLLs, updates registry overrides.
    nonisolated func switchBackend(bottleId: String, backend: GraphicsBackend) -> SwitchBackendResult? {
        guard let ptr = managerPtr else { return nil }
        let jsonPtr = bottleId.withCString { bidCStr in
            backend.rawValue.withCString { backendCStr in
                cauldron_switch_backend(ptr, bidCStr, backendCStr)
            }
        }
        guard let resultString = consumeCString(jsonPtr) else { return nil }
        return decodeJSON(resultString)
    }

    // MARK: - Private Helpers

    /// Converts a C string pointer to a Swift String, then frees the C string.
    private nonisolated func consumeCString(_ ptr: UnsafeMutablePointer<CChar>?) -> String? {
        guard let ptr = ptr else { return nil }
        let string = String(cString: ptr)
        cauldron_free_string(ptr)
        return string
    }

    private nonisolated func decodeJSON<T: Decodable>(_ json: String) -> T? {
        guard let data = json.data(using: .utf8) else { return nil }
        do {
            let decoder = JSONDecoder()
            decoder.keyDecodingStrategy = .convertFromSnakeCase
            return try decoder.decode(T.self, from: data)
        } catch {
            print("[CauldronBridge] JSON decode error: \(error)")
            return nil
        }
    }

    private static func appSupportDirectory() -> String {
        let fm = FileManager.default
        let appSupport = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let cauldronDir = appSupport.appendingPathComponent("Cauldron", isDirectory: true)
        try? fm.createDirectory(at: cauldronDir, withIntermediateDirectories: true)
        return cauldronDir.path
    }
}
