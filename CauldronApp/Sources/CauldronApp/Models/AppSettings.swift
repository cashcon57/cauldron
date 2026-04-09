import SwiftUI

@MainActor
@Observable
final class AppSettings {
    static let shared = AppSettings()

    // MARK: - Active Profile

    var activeProfile: ConfigProfile {
        get {
            guard let raw = UserDefaults.standard.string(forKey: "activeProfile"),
                  let value = ConfigProfile(rawValue: raw) else { return .stable }
            return value
        }
        set {
            UserDefaults.standard.set(newValue.rawValue, forKey: "activeProfile")
        }
    }

    /// Apply a profile's preset values to all global settings.
    func applyProfile(_ profile: ConfigProfile) {
        let p = profile.preset
        activeProfile = profile
        defaultGraphicsBackend = p.graphicsBackend
        rosettaX87Enabled = p.rosettaX87Enabled
        asyncShaderCompilation = p.asyncShaderCompilation
        metalFXSpatialUpscaling = p.metalFXSpatialUpscaling
        dxrRayTracing = p.dxrRayTracing
        moltenVKArgumentBuffers = p.moltenVKArgumentBuffers
        metalPerformanceHUD = p.metalPerformanceHUD
        enableAutoSync = p.enableAutoSync
        syncInterval = p.syncInterval
        showNightlyPatches = p.showNightlyPatches
        enablePerformanceMonitoring = p.enablePerformanceMonitoring
        frameTimingOverlay = p.frameTimingOverlay
        autoApplyGamePatches = p.autoApplyGamePatches
    }

    /// Snapshot the current global settings as a ProfileSettings for comparison.
    var currentAsProfileSettings: ProfileSettings {
        ProfileSettings(
            graphicsBackend: defaultGraphicsBackend,
            msyncEnabled: true, // always on at global level
            esyncEnabled: true,
            rosettaX87Enabled: rosettaX87Enabled,
            asyncShaderCompilation: asyncShaderCompilation,
            metalFXSpatialUpscaling: metalFXSpatialUpscaling,
            dxrRayTracing: dxrRayTracing,
            moltenVKArgumentBuffers: moltenVKArgumentBuffers,
            metalPerformanceHUD: metalPerformanceHUD,
            fsrEnabled: false,
            largeAddressAware: false,
            autoApplyGamePatches: autoApplyGamePatches,
            enableAutoSync: enableAutoSync,
            syncInterval: syncInterval,
            showNightlyPatches: showNightlyPatches,
            enablePerformanceMonitoring: enablePerformanceMonitoring,
            frameTimingOverlay: frameTimingOverlay
        )
    }

    /// Check if current global settings match the active profile exactly.
    var globalMatchesProfile: Bool {
        currentAsProfileSettings == activeProfile.preset
    }

    // MARK: - General

    var defaultWineVersion: String {
        get { UserDefaults.standard.string(forKey: "defaultWineVersion") ?? "cauldron-11.6" }
        set { UserDefaults.standard.set(newValue, forKey: "defaultWineVersion") }
    }

    var defaultGraphicsBackend: GraphicsBackend {
        get {
            guard let raw = UserDefaults.standard.string(forKey: "defaultGraphicsBackend"),
                  let value = GraphicsBackend(rawValue: raw) else { return .auto }
            return value
        }
        set { UserDefaults.standard.set(newValue.rawValue, forKey: "defaultGraphicsBackend") }
    }

    var bottlesDirectory: String {
        get { UserDefaults.standard.string(forKey: "bottlesDirectory") ?? defaultBottlesPath }
        set { UserDefaults.standard.set(newValue, forKey: "bottlesDirectory") }
    }

    var autoLaunchSteam: Bool {
        get { UserDefaults.standard.bool(forKey: "autoLaunchSteam") }
        set { UserDefaults.standard.set(newValue, forKey: "autoLaunchSteam") }
    }

    var checkForUpdates: Bool {
        get {
            // Self-builds don't auto-update — we didn't sign them, so pushing
            // updates would break codesigning. This isn't a restriction on the
            // software, just on the delivery mechanism.
            guard BuildChannel.isOfficialBuild else { return false }
            return UserDefaults.standard.object(forKey: "checkForUpdates") as? Bool ?? true
        }
        set {
            guard BuildChannel.isOfficialBuild else { return }
            UserDefaults.standard.set(newValue, forKey: "checkForUpdates")
        }
    }

    // MARK: - Graphics

    var metalPerformanceHUD: Bool {
        get { UserDefaults.standard.bool(forKey: "metalPerformanceHUD") }
        set {
            UserDefaults.standard.set(newValue, forKey: "metalPerformanceHUD")
            // Metal HUD via global defaults — works through Wine/Rosetta/DXMT because
            // Metal reads CFPreferences, not process env vars.
            UserDefaults.standard.set(newValue, forKey: "MetalForceHudEnabled")
            let globalDefaults = UserDefaults(suiteName: UserDefaults.globalDomain)
            globalDefaults?.set(newValue, forKey: "MetalForceHudEnabled")
        }
    }

    var asyncShaderCompilation: Bool {
        get { UserDefaults.standard.object(forKey: "asyncShaderCompilation") as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: "asyncShaderCompilation") }
    }

    var metalFXSpatialUpscaling: Bool {
        get { UserDefaults.standard.bool(forKey: "metalFXSpatialUpscaling") }
        set { UserDefaults.standard.set(newValue, forKey: "metalFXSpatialUpscaling") }
    }

    var dxrRayTracing: Bool {
        get { UserDefaults.standard.bool(forKey: "dxrRayTracing") }
        set { UserDefaults.standard.set(newValue, forKey: "dxrRayTracing") }
    }

    var moltenVKArgumentBuffers: Bool {
        get { UserDefaults.standard.bool(forKey: "moltenVKArgumentBuffers") }
        set { UserDefaults.standard.set(newValue, forKey: "moltenVKArgumentBuffers") }
    }

    // MARK: - RosettaX87

    var rosettaX87Enabled: Bool {
        get { UserDefaults.standard.bool(forKey: "rosettaX87Enabled") }
        set { UserDefaults.standard.set(newValue, forKey: "rosettaX87Enabled") }
    }

    // MARK: - Rendering

    var fsrEnabled: Bool {
        get { UserDefaults.standard.bool(forKey: "fsrEnabled") }
        set { UserDefaults.standard.set(newValue, forKey: "fsrEnabled") }
    }

    var fsrStrength: Int {
        get { UserDefaults.standard.object(forKey: "fsrStrength") as? Int ?? 2 }
        set { UserDefaults.standard.set(newValue, forKey: "fsrStrength") }
    }

    var highResolutionMode: Bool {
        get { UserDefaults.standard.bool(forKey: "highResolutionMode") }
        set { UserDefaults.standard.set(newValue, forKey: "highResolutionMode") }
    }

    var frameRateLimit: Int {
        get { UserDefaults.standard.object(forKey: "frameRateLimit") as? Int ?? 0 }
        set { UserDefaults.standard.set(newValue, forKey: "frameRateLimit") }
    }

    // MARK: - Compatibility

    var heapZeroMemory: Bool {
        get { UserDefaults.standard.bool(forKey: "heapZeroMemory") }
        set { UserDefaults.standard.set(newValue, forKey: "heapZeroMemory") }
    }

    // MARK: - Game Patches

    var autoApplyGamePatches: Bool {
        get { UserDefaults.standard.bool(forKey: "autoApplyGamePatches") }
        set { UserDefaults.standard.set(newValue, forKey: "autoApplyGamePatches") }
    }

    // MARK: - Sync

    var enableAutoSync: Bool {
        get { UserDefaults.standard.object(forKey: "enableAutoSync") as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: "enableAutoSync") }
    }

    var syncInterval: SyncInterval {
        get {
            guard let raw = UserDefaults.standard.string(forKey: "syncInterval"),
                  let value = SyncInterval(rawValue: raw) else { return .sixHours }
            return value
        }
        set { UserDefaults.standard.set(newValue.rawValue, forKey: "syncInterval") }
    }

    var protonRepositoryURL: String {
        get { UserDefaults.standard.string(forKey: "protonRepositoryURL") ?? "https://github.com/ValveSoftware/Proton" }
        set { UserDefaults.standard.set(newValue, forKey: "protonRepositoryURL") }
    }

    var showNightlyPatches: Bool {
        get { UserDefaults.standard.bool(forKey: "showNightlyPatches") }
        set { UserDefaults.standard.set(newValue, forKey: "showNightlyPatches") }
    }

    // MARK: - Performance

    var enablePerformanceMonitoring: Bool {
        get { UserDefaults.standard.bool(forKey: "enablePerformanceMonitoring") }
        set { UserDefaults.standard.set(newValue, forKey: "enablePerformanceMonitoring") }
    }

    var frameTimingOverlay: Bool {
        get { UserDefaults.standard.bool(forKey: "frameTimingOverlay") }
        set { UserDefaults.standard.set(newValue, forKey: "frameTimingOverlay") }
    }

    var logLevel: LogLevel {
        get {
            guard let raw = UserDefaults.standard.string(forKey: "logLevel"),
                  let value = LogLevel(rawValue: raw) else { return .normal }
            return value
        }
        set { UserDefaults.standard.set(newValue.rawValue, forKey: "logLevel") }
    }

    var logDirectory: String {
        get { UserDefaults.standard.string(forKey: "logDirectory") ?? defaultLogPath }
        set { UserDefaults.standard.set(newValue, forKey: "logDirectory") }
    }

    // MARK: - Defaults

    private var defaultBottlesPath: String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        return "\(home)/Library/Containers/Cauldron/Bottles"
    }

    private var defaultLogPath: String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        return "\(home)/Library/Logs/Cauldron"
    }

    // MARK: - Available Wine Versions
    // Dynamic versions come from CauldronBridge.shared.getWineVersions().
    // This fallback list is only used if the bridge is unavailable.

    static let availableWineVersions: [String] = [
        "cauldron-11.6",
        "wine-10.0",
        "wine-9.0",
    ]

    private init() {}
}

// MARK: - Supporting Types

enum SyncInterval: String, CaseIterable, Codable {
    case oneHour = "1h"
    case sixHours = "6h"
    case twelveHours = "12h"
    case twentyFourHours = "24h"

    var displayName: String {
        switch self {
        case .oneHour: return "Every hour"
        case .sixHours: return "Every 6 hours"
        case .twelveHours: return "Every 12 hours"
        case .twentyFourHours: return "Every 24 hours"
        }
    }
}

enum LogLevel: String, CaseIterable {
    case quiet, normal, verbose, debug

    var displayName: String {
        rawValue.capitalized
    }
}
