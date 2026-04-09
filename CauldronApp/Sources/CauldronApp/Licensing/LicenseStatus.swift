import Foundation

enum LicenseStatus: Equatable {
    /// Self-build from source. No restrictions, no activation.
    case community

    /// Active trial with days remaining.
    case trial(daysRemaining: Int)

    /// Activated via code. Permanent, fully offline.
    case activated

    /// Trial expired, not activated.
    case expired

    var canLaunchGames: Bool {
        switch self {
        case .community, .trial, .activated: return true
        case .expired: return false
        }
    }

    var displayName: String {
        switch self {
        case .community: return "Community Build"
        case .trial(let days): return "Trial (\(days) day\(days == 1 ? "" : "s") remaining)"
        case .activated: return "Activated"
        case .expired: return "Trial Expired"
        }
    }
}
