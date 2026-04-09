import SwiftUI

// MARK: - Profile Tier

/// The three optimization profiles that configure all settings at once.
enum ConfigProfile: String, CaseIterable, Codable {
    case stable = "stable"
    case preview = "preview"
    case bleedingEdge = "bleeding_edge"

    var displayName: String {
        switch self {
        case .stable: return "Stable"
        case .preview: return "Preview"
        case .bleedingEdge: return "Bleeding Edge"
        }
    }

    var icon: String {
        switch self {
        case .stable: return "shield.checkered"
        case .preview: return "eye"
        case .bleedingEdge: return "flame.fill"
        }
    }

    var tintColor: Color {
        switch self {
        case .stable: return .green
        case .preview: return .blue
        case .bleedingEdge: return .orange
        }
    }

    var tagline: String {
        switch self {
        case .stable:
            return "Tested, conservative settings. Best for most users."
        case .preview:
            return "Newer features enabled. May have rough edges."
        case .bleedingEdge:
            return "Everything enabled. Maximum performance, latest patches. Here be dragons."
        }
    }

    /// The full set of option values this profile prescribes.
    var preset: ProfileSettings {
        switch self {
        case .stable:
            return ProfileSettings(
                graphicsBackend: .auto,
                msyncEnabled: true,
                esyncEnabled: true,
                rosettaX87Enabled: false,
                asyncShaderCompilation: true,
                metalFXSpatialUpscaling: false,
                dxrRayTracing: false,
                moltenVKArgumentBuffers: false,
                metalPerformanceHUD: false,
                fsrEnabled: false,
                largeAddressAware: false,
                autoApplyGamePatches: false,
                enableAutoSync: true,
                syncInterval: .twentyFourHours,
                showNightlyPatches: false,
                enablePerformanceMonitoring: false,
                frameTimingOverlay: false
            )
        case .preview:
            return ProfileSettings(
                graphicsBackend: .auto,
                msyncEnabled: true,
                esyncEnabled: true,
                rosettaX87Enabled: true,
                asyncShaderCompilation: true,
                metalFXSpatialUpscaling: true,
                dxrRayTracing: false,
                moltenVKArgumentBuffers: false,
                metalPerformanceHUD: false,
                fsrEnabled: false,
                largeAddressAware: false,
                autoApplyGamePatches: false,
                enableAutoSync: true,
                syncInterval: .sixHours,
                showNightlyPatches: false,
                enablePerformanceMonitoring: false,
                frameTimingOverlay: false
            )
        case .bleedingEdge:
            return ProfileSettings(
                graphicsBackend: .auto,
                msyncEnabled: true,
                esyncEnabled: true,
                rosettaX87Enabled: true,
                asyncShaderCompilation: true,
                metalFXSpatialUpscaling: true,
                dxrRayTracing: true,
                moltenVKArgumentBuffers: false,
                metalPerformanceHUD: false,
                fsrEnabled: false,
                largeAddressAware: false,
                autoApplyGamePatches: true,
                enableAutoSync: true,
                syncInterval: .oneHour,
                showNightlyPatches: true,
                enablePerformanceMonitoring: true,
                frameTimingOverlay: false
            )
        }
    }
}

// MARK: - ProfileSettings

/// Every tunable option that a profile controls.
/// Used both as the "preset definition" and as "per-game override snapshot".
struct ProfileSettings: Equatable, Codable {
    var graphicsBackend: GraphicsBackend
    var msyncEnabled: Bool
    var esyncEnabled: Bool
    var rosettaX87Enabled: Bool
    var asyncShaderCompilation: Bool
    var metalFXSpatialUpscaling: Bool
    var dxrRayTracing: Bool
    var moltenVKArgumentBuffers: Bool
    var metalPerformanceHUD: Bool
    var fsrEnabled: Bool
    var largeAddressAware: Bool
    var autoApplyGamePatches: Bool
    var enableAutoSync: Bool
    var syncInterval: SyncInterval
    var showNightlyPatches: Bool
    var enablePerformanceMonitoring: Bool
    var frameTimingOverlay: Bool

    /// List of settings that differ from a reference profile.
    func differences(from reference: ProfileSettings) -> [SettingDifference] {
        var diffs: [SettingDifference] = []

        if graphicsBackend != reference.graphicsBackend {
            diffs.append(.init(name: "Graphics Backend",
                               current: graphicsBackend.displayName,
                               expected: reference.graphicsBackend.displayName))
        }
        if msyncEnabled != reference.msyncEnabled {
            diffs.append(.init(name: "MSync", current: msyncEnabled.label, expected: reference.msyncEnabled.label))
        }
        if esyncEnabled != reference.esyncEnabled {
            diffs.append(.init(name: "ESync", current: esyncEnabled.label, expected: reference.esyncEnabled.label))
        }
        if rosettaX87Enabled != reference.rosettaX87Enabled {
            diffs.append(.init(name: "RosettaX87", current: rosettaX87Enabled.label, expected: reference.rosettaX87Enabled.label))
        }
        if asyncShaderCompilation != reference.asyncShaderCompilation {
            diffs.append(.init(name: "Async Shaders", current: asyncShaderCompilation.label, expected: reference.asyncShaderCompilation.label))
        }
        if metalFXSpatialUpscaling != reference.metalFXSpatialUpscaling {
            diffs.append(.init(name: "MetalFX Upscaling", current: metalFXSpatialUpscaling.label, expected: reference.metalFXSpatialUpscaling.label))
        }
        if dxrRayTracing != reference.dxrRayTracing {
            diffs.append(.init(name: "DXR Ray Tracing", current: dxrRayTracing.label, expected: reference.dxrRayTracing.label))
        }
        if moltenVKArgumentBuffers != reference.moltenVKArgumentBuffers {
            diffs.append(.init(name: "MoltenVK Arg Buffers", current: moltenVKArgumentBuffers.label, expected: reference.moltenVKArgumentBuffers.label))
        }
        if fsrEnabled != reference.fsrEnabled {
            diffs.append(.init(name: "FSR", current: fsrEnabled.label, expected: reference.fsrEnabled.label))
        }
        if largeAddressAware != reference.largeAddressAware {
            diffs.append(.init(name: "Large Address Aware", current: largeAddressAware.label, expected: reference.largeAddressAware.label))
        }
        if autoApplyGamePatches != reference.autoApplyGamePatches {
            diffs.append(.init(name: "Auto-Apply Game Patches", current: autoApplyGamePatches.label, expected: reference.autoApplyGamePatches.label))
        }
        if showNightlyPatches != reference.showNightlyPatches {
            diffs.append(.init(name: "Nightly Patches", current: showNightlyPatches.label, expected: reference.showNightlyPatches.label))
        }
        if metalPerformanceHUD != reference.metalPerformanceHUD {
            diffs.append(.init(name: "Metal HUD", current: metalPerformanceHUD.label, expected: reference.metalPerformanceHUD.label))
        }
        if enablePerformanceMonitoring != reference.enablePerformanceMonitoring {
            diffs.append(.init(name: "Performance Monitoring", current: enablePerformanceMonitoring.label, expected: reference.enablePerformanceMonitoring.label))
        }
        if frameTimingOverlay != reference.frameTimingOverlay {
            diffs.append(.init(name: "Frame Timing Overlay", current: frameTimingOverlay.label, expected: reference.frameTimingOverlay.label))
        }
        if syncInterval != reference.syncInterval {
            diffs.append(.init(name: "Sync Interval", current: syncInterval.displayName, expected: reference.syncInterval.displayName))
        }

        return diffs
    }
}

// MARK: - SettingDifference

/// One setting that diverges from the active profile.
struct SettingDifference: Identifiable {
    var id: String { name }
    let name: String
    let current: String
    let expected: String
}

// MARK: - Helpers

private extension Bool {
    var label: String { self ? "On" : "Off" }
}
