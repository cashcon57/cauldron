import Foundation

/// Represents a Wine version available for download or already installed.
struct WineVersionInfo: Codable, Identifiable {
    var id: String { version }

    let version: String
    let url: String
    let sha256: String?
    let installed: Bool
    let path: String
    let category: String
}

/// Result of a Wine download operation from the FFI bridge.
struct WineDownloadResult: Codable {
    let success: Bool
    let path: String?
    let version: String?
    let error: String?
}
