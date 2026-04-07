import Foundation

struct PatchAnalysis: Codable, Identifiable {
    let hash: String
    let appliesCleanly: Bool?
    let conflictFiles: [String]
    let affectedDlls: [String]
    let impact: String
    let impactReason: String
    let linesAdded: Int
    let linesRemoved: Int
    let affectedGames: [String]
    let protondbRating: String?
    let canAutoAdapt: Bool?
    let adaptationTransformCount: Int?
    let adaptationConfidence: String?
    let adaptationWarnings: [String]?
    let moddingImpact: [String]?
    let suggestedAction: String?

    var id: String { hash }
}
