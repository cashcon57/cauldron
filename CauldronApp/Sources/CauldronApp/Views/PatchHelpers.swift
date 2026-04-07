import SwiftUI

// MARK: - Shared Patch Display Helpers

enum PatchDisplayHelpers {
    static func impactColor(_ impact: String) -> Color {
        switch impact {
        case "high risk": return .red
        case "medium risk": return .orange
        case "low risk": return .green
        default: return .secondary
        }
    }

    static func protondbColor(_ rating: String) -> Color {
        switch rating.lowercased() {
        case "platinum": return .green
        case "gold": return .yellow
        case "silver": return .gray
        case "bronze": return .orange
        case "borked": return .red
        default: return .secondary
        }
    }

    static func statusIcon(_ status: String) -> String {
        switch status {
        case "applied": return "checkmark.circle.fill"
        case "pending": return "clock.fill"
        case "skipped": return "arrow.uturn.right"
        case "conflicted": return "exclamationmark.triangle.fill"
        default: return "questionmark.circle"
        }
    }

    static func statusColor(_ status: String) -> Color {
        switch status {
        case "applied": return .green
        case "pending": return .orange
        case "skipped": return .gray
        case "conflicted": return .red
        default: return .secondary
        }
    }

    static func classificationColor(_ classification: String) -> Color {
        switch classification {
        case "WineApiFix": return .blue
        case "DxvkFix": return .purple
        case "Vkd3dFix": return .orange
        case "GameConfig": return .green
        case "KernelWorkaround": return .red
        case "SteamIntegration": return .cyan
        case "BuildSystem": return .gray
        default: return .secondary
        }
    }

    static func classificationDisplayName(_ key: String, plural: Bool = false) -> String {
        switch key {
        case "WineApiFix": return plural ? "Wine API Fixes" : "Wine API Fix"
        case "DxvkFix": return plural ? "DXVK Fixes" : "DXVK Fix"
        case "Vkd3dFix": return plural ? "VKD3D-Proton Fixes" : "VKD3D-Proton Fix"
        case "GameConfig": return "Game Configuration"
        case "KernelWorkaround": return plural ? "Kernel Workarounds" : "Kernel Workaround"
        case "SteamIntegration": return "Steam Integration"
        case "BuildSystem": return plural ? "Build System & Dependencies" : "Build System"
        default: return plural ? "Other" : "Unclassified"
        }
    }

    static func transferabilityLabel(_ transferability: String) -> String {
        switch transferability {
        case "High": return "Portable"
        case "Medium": return "Needs review"
        case "Low": return "Needs adaptation"
        case "None": return "Not applicable"
        default: return transferability
        }
    }

    static func transferabilityColor(_ transferability: String) -> Color {
        switch transferability {
        case "High": return .green
        case "Medium": return .yellow
        case "Low": return .orange
        case "None": return .gray
        default: return .secondary
        }
    }

    static func sourceLabel(_ source: String) -> String {
        switch source {
        case "proton": return "Proton"
        case "crossover": return "CrossOver"
        default: return source.capitalized
        }
    }

    static func sourceColor(_ source: String) -> Color {
        switch source {
        case "proton": return .blue
        case "crossover": return .purple
        default: return .secondary
        }
    }
}
