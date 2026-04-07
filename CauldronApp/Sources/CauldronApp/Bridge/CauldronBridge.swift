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

    nonisolated func launchExe(bottleId: String, exePath: String, backend: String) -> Bool {
        guard let ptr = managerPtr else { return false }
        let result = bottleId.withCString { bidCStr in
            exePath.withCString { exeCStr in
                backend.withCString { backendCStr in
                    cauldron_launch_exe(ptr, bidCStr, exeCStr, backendCStr)
                }
            }
        }
        return result == 0
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
