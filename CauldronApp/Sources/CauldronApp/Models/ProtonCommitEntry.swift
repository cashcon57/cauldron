import Foundation

struct ProtonCommitEntry: Codable, Identifiable {
    let hash: String
    let message: String
    let author: String
    let timestamp: String
    let affectedFiles: String
    let classification: String
    let transferability: String
    let applied: Bool
    let source: String

    var id: String { hash }
}

struct PatchActionResult: Codable {
    let success: Bool
    let error: String?
    let outcome: String?
    let filesChanged: Int?
}
