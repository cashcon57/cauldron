import SwiftUI

// MARK: - Per-Game Settings

/// Per-game configuration overrides. Each field is optional — `nil` means
/// "inherit from global profile". Any non-nil value overrides the global.
@MainActor
@Observable
final class PerGameSettings {

    /// Unique key for this game (steam app ID or exe hash).
    let gameKey: String

    init(gameKey: String) {
        self.gameKey = gameKey
        load()
    }

    // MARK: - Override Fields (nil = inherit global)

    var graphicsBackend: GraphicsBackend? = nil
    var msyncEnabled: Bool? = nil
    var esyncEnabled: Bool? = nil
    var rosettaX87Enabled: Bool? = nil
    var asyncShaderCompilation: Bool? = nil
    var metalFXSpatialUpscaling: Bool? = nil
    var dxrRayTracing: Bool? = nil
    var moltenVKArgumentBuffers: Bool? = nil
    var metalPerformanceHUD: Bool? = nil
    var fsrEnabled: Bool? = nil
    var largeAddressAware: Bool? = nil
    var autoApplyGamePatches: Bool? = nil
    var highResolutionMode: Bool? = nil
    var frameRateLimit: Int? = nil
    var heapZeroMemory: Bool? = nil

    /// Whether this game has any custom overrides at all.
    var hasOverrides: Bool {
        graphicsBackend != nil || msyncEnabled != nil || esyncEnabled != nil ||
        rosettaX87Enabled != nil || asyncShaderCompilation != nil ||
        metalFXSpatialUpscaling != nil || dxrRayTracing != nil ||
        moltenVKArgumentBuffers != nil || metalPerformanceHUD != nil ||
        fsrEnabled != nil || largeAddressAware != nil || autoApplyGamePatches != nil ||
        highResolutionMode != nil || frameRateLimit != nil || heapZeroMemory != nil
    }

    /// Build the effective settings for this game by merging overrides onto the global profile.
    func effectiveSettings(global: ProfileSettings) -> ProfileSettings {
        ProfileSettings(
            graphicsBackend: graphicsBackend ?? global.graphicsBackend,
            msyncEnabled: msyncEnabled ?? global.msyncEnabled,
            esyncEnabled: esyncEnabled ?? global.esyncEnabled,
            rosettaX87Enabled: rosettaX87Enabled ?? global.rosettaX87Enabled,
            asyncShaderCompilation: asyncShaderCompilation ?? global.asyncShaderCompilation,
            metalFXSpatialUpscaling: metalFXSpatialUpscaling ?? global.metalFXSpatialUpscaling,
            dxrRayTracing: dxrRayTracing ?? global.dxrRayTracing,
            moltenVKArgumentBuffers: moltenVKArgumentBuffers ?? global.moltenVKArgumentBuffers,
            metalPerformanceHUD: metalPerformanceHUD ?? global.metalPerformanceHUD,
            fsrEnabled: fsrEnabled ?? global.fsrEnabled,
            largeAddressAware: largeAddressAware ?? global.largeAddressAware,
            autoApplyGamePatches: autoApplyGamePatches ?? global.autoApplyGamePatches,
            enableAutoSync: global.enableAutoSync,
            syncInterval: global.syncInterval,
            showNightlyPatches: global.showNightlyPatches,
            enablePerformanceMonitoring: global.enablePerformanceMonitoring,
            frameTimingOverlay: global.frameTimingOverlay
        )
    }

    /// Detect differences between this game's effective settings and the
    /// active global profile preset.
    func mismatchesFromProfile(_ profile: ConfigProfile) -> [SettingDifference] {
        let global = profile.preset
        let effective = effectiveSettings(global: global)
        return effective.differences(from: global)
    }

    /// Reset all overrides so the game inherits everything from the global profile.
    func resetToGlobal() {
        graphicsBackend = nil
        msyncEnabled = nil
        esyncEnabled = nil
        rosettaX87Enabled = nil
        asyncShaderCompilation = nil
        metalFXSpatialUpscaling = nil
        dxrRayTracing = nil
        moltenVKArgumentBuffers = nil
        metalPerformanceHUD = nil
        fsrEnabled = nil
        largeAddressAware = nil
        autoApplyGamePatches = nil
        save()
    }

    // MARK: - Persistence (UserDefaults)

    func save() {
        let defaults = UserDefaults.standard
        let prefix = "game_\(gameKey)_"

        defaults.set(graphicsBackend?.rawValue, forKey: prefix + "graphicsBackend")
        saveOptionalBool(msyncEnabled, key: prefix + "msyncEnabled")
        saveOptionalBool(esyncEnabled, key: prefix + "esyncEnabled")
        saveOptionalBool(rosettaX87Enabled, key: prefix + "rosettaX87Enabled")
        saveOptionalBool(asyncShaderCompilation, key: prefix + "asyncShaderCompilation")
        saveOptionalBool(metalFXSpatialUpscaling, key: prefix + "metalFXSpatialUpscaling")
        saveOptionalBool(dxrRayTracing, key: prefix + "dxrRayTracing")
        saveOptionalBool(moltenVKArgumentBuffers, key: prefix + "moltenVKArgumentBuffers")
        saveOptionalBool(metalPerformanceHUD, key: prefix + "metalPerformanceHUD")
        saveOptionalBool(fsrEnabled, key: prefix + "fsrEnabled")
        saveOptionalBool(largeAddressAware, key: prefix + "largeAddressAware")
        saveOptionalBool(autoApplyGamePatches, key: prefix + "autoApplyGamePatches")
    }

    private func load() {
        let defaults = UserDefaults.standard
        let prefix = "game_\(gameKey)_"

        if let raw = defaults.string(forKey: prefix + "graphicsBackend") {
            graphicsBackend = GraphicsBackend(rawValue: raw)
        }
        msyncEnabled = loadOptionalBool(key: prefix + "msyncEnabled")
        esyncEnabled = loadOptionalBool(key: prefix + "esyncEnabled")
        rosettaX87Enabled = loadOptionalBool(key: prefix + "rosettaX87Enabled")
        asyncShaderCompilation = loadOptionalBool(key: prefix + "asyncShaderCompilation")
        metalFXSpatialUpscaling = loadOptionalBool(key: prefix + "metalFXSpatialUpscaling")
        dxrRayTracing = loadOptionalBool(key: prefix + "dxrRayTracing")
        moltenVKArgumentBuffers = loadOptionalBool(key: prefix + "moltenVKArgumentBuffers")
        metalPerformanceHUD = loadOptionalBool(key: prefix + "metalPerformanceHUD")
        fsrEnabled = loadOptionalBool(key: prefix + "fsrEnabled")
        largeAddressAware = loadOptionalBool(key: prefix + "largeAddressAware")
        autoApplyGamePatches = loadOptionalBool(key: prefix + "autoApplyGamePatches")
    }

    private func saveOptionalBool(_ value: Bool?, key: String) {
        if let v = value {
            UserDefaults.standard.set(v, forKey: key)
        } else {
            UserDefaults.standard.removeObject(forKey: key)
        }
    }

    private func loadOptionalBool(key: String) -> Bool? {
        UserDefaults.standard.object(forKey: key) as? Bool
    }
}
