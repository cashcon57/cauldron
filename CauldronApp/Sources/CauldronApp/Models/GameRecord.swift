import Foundation

struct GameRecord: Identifiable, Codable, Hashable {
    var id: String { "\(steamAppId ?? 0)-\(title)" }
    let steamAppId: Int?
    let title: String
    let backend: String
    let compatStatus: String
    let knownIssues: String
    let notes: String
}
