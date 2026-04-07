import Foundation

struct Bottle: Identifiable, Codable, Hashable {
    let id: String
    var name: String
    let path: String
    var wineVersion: String
    var graphicsBackend: String
    let createdAt: String
    var envOverrides: [String: String]?
}
