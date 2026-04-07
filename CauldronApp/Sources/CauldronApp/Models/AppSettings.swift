import SwiftUI

@MainActor
@Observable
final class AppSettings {
    static let shared = AppSettings()

    // MARK: - General

    var defaultWineVersion: String {
        get { UserDefaults.standard.string(forKey: "defaultWineVersion") ?? "wine-10.0" }
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
        get { UserDefaults.standard.object(forKey: "checkForUpdates") as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: "checkForUpdates") }
    }

    // MARK: - Graphics

    var metalPerformanceHUD: Bool {
        get { UserDefaults.standard.bool(forKey: "metalPerformanceHUD") }
        set { UserDefaults.standard.set(newValue, forKey: "metalPerformanceHUD") }
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
        "wine-10.0",
        "wine-9.0",
    ]

    private init() {}
}

// MARK: - Supporting Types

enum SyncInterval: String, CaseIterable {
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
