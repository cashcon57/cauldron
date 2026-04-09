import Foundation
import CryptoKit

/// Validates Ed25519-signed JWT receipts.
/// The public key is embedded at compile time in BuildChannel.swift.
/// The private key exists only on the activation server.
enum JWTValidator {

    struct Claims: Codable {
        let sub: String        // machine_id hash
        let type: String       // "paid" or "trial"
        let iat: TimeInterval  // issued at (unix)
        let exp: TimeInterval  // expires at (unix)
        let cid: String?       // stripe customer id (nil for trial)
    }

    /// Validate a JWT string and return its claims if the signature is valid.
    static func validate(_ jwt: String, publicKey: Curve25519.Signing.PublicKey) -> Claims? {
        let parts = jwt.split(separator: ".")
        guard parts.count == 3 else { return nil }

        let headerPayload = "\(parts[0]).\(parts[1])"
        guard let signatureData = base64URLDecode(String(parts[2])),
              let payloadData = base64URLDecode(String(parts[1])) else {
            return nil
        }

        // Verify Ed25519 signature
        guard publicKey.isValidSignature(signatureData, for: Data(headerPayload.utf8)) else {
            return nil
        }

        // Decode claims
        guard let claims = try? JSONDecoder().decode(Claims.self, from: payloadData) else {
            return nil
        }

        return claims
    }

    /// Create a locally-signed trial receipt.
    /// Uses a trial-specific key embedded in the binary.
    static func createTrialReceipt(machineId: String, trialDays: Int = 14) -> String? {
        guard let keyData = base64URLDecode(BuildChannel.trialSigningKey ?? "") else {
            return nil
        }
        guard let privateKey = try? Curve25519.Signing.PrivateKey(rawRepresentation: keyData) else {
            return nil
        }

        let now = Date().timeIntervalSince1970
        let claims = Claims(
            sub: machineId,
            type: "trial",
            iat: now,
            exp: now + Double(trialDays * 86400),
            cid: nil
        )

        guard let claimsData = try? JSONEncoder().encode(claims) else { return nil }

        let header = base64URLEncode(Data(#"{"alg":"EdDSA","typ":"JWT"}"#.utf8))
        let payload = base64URLEncode(claimsData)
        let signingInput = "\(header).\(payload)"

        guard let signature = try? privateKey.signature(for: Data(signingInput.utf8)) else {
            return nil
        }

        return "\(signingInput).\(base64URLEncode(signature))"
    }

    // MARK: - Base64URL

    static func base64URLDecode(_ string: String) -> Data? {
        var base64 = string
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        while base64.count % 4 != 0 { base64.append("=") }
        return Data(base64Encoded: base64)
    }

    static func base64URLEncode(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}
