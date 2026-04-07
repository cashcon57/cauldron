import Foundation

struct SyncStatus: Codable {
    let lastSyncTimestamp: String?
    let lastCommitHash: String?
    let totalCommitsProcessed: Int
    let commitsApplied: Int
    let commitsPending: Int
    let commitsSkipped: Int
    let lastError: String?
}
