import Foundation
import CryptoKit
import IOKit

enum MachineIdentifier {
    /// Returns a stable SHA-256 hash of the hardware UUID.
    /// Used during activation so the server can allow re-activation
    /// on the same machine without consuming the code twice.
    static var id: String {
        let uuid = platformUUID ?? "unknown-machine"
        let hash = SHA256.hash(data: Data(uuid.utf8))
        return hash.map { String(format: "%02x", $0) }.joined()
    }

    private static var platformUUID: String? {
        let service = IOServiceGetMatchingService(
            kIOMainPortDefault,
            IOServiceMatching("IOPlatformExpertDevice")
        )
        guard service != IO_OBJECT_NULL else { return nil }
        defer { IOObjectRelease(service) }

        let key = kIOPlatformUUIDKey as CFString
        guard let uuid = IORegistryEntryCreateCFProperty(service, key, kCFAllocatorDefault, 0)?
            .takeRetainedValue() as? String else { return nil }
        return uuid
    }
}
