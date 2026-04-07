import Foundation

struct DiscoveredBottle: Identifiable, Codable, Hashable {
    var id: String { path }
    let name: String
    let path: String
    let source: String
    let wineVersion: String
    let sizeBytes: Int64
    let hasSteam: Bool
    let gameCount: Int
    let graphicsBackend: String
}
