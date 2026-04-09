import Foundation
import CryptoKit

/// Manages activation state for Cauldron.
///
/// Community builds: always `.community`, no checks.
/// Official builds: checks Keychain for a signed receipt (trial or paid).
/// One-time online activation, then fully offline forever.
@MainActor
@Observable
final class LicenseManager {
    private(set) var status: LicenseStatus = .expired
    var activationError: String?

    private let activationURL = "https://api.cauldron.app/api/activate"

    init() {
        // Community builds — no activation, no restrictions
        guard BuildChannel.requiresActivation else {
            status = .community
            return
        }
        loadAndValidateReceipt()
    }

    // MARK: - Trial

    func startTrial() {
        guard BuildChannel.requiresActivation else { return }

        let machineId = MachineIdentifier.id
        guard let jwt = JWTValidator.createTrialReceipt(machineId: machineId) else {
            activationError = "Failed to create trial receipt"
            return
        }

        let receipt = ActivationReceipt(
            jwt: jwt,
            lastSeenTime: Date().timeIntervalSince1970,
            activatedAt: Date().timeIntervalSince1970
        )
        saveReceipt(receipt)
        loadAndValidateReceipt()
    }

    // MARK: - Code Activation

    func activate(code: String) async {
        guard BuildChannel.requiresActivation else { return }
        activationError = nil

        let cleanCode = code.trimmingCharacters(in: .whitespaces).uppercased()
        guard cleanCode.count == 6 else {
            activationError = "Code must be 6 characters"
            return
        }

        let machineId = MachineIdentifier.id
        let body: [String: String] = ["code": cleanCode, "machine_id": machineId]

        guard let bodyData = try? JSONSerialization.data(withJSONObject: body) else {
            activationError = "Internal error"
            return
        }

        var request = URLRequest(url: URL(string: activationURL)!)
        request.httpMethod = "POST"
        request.httpBody = bodyData
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.timeoutInterval = 15

        do {
            let (data, response) = try await URLSession.shared.data(for: request)

            guard let httpResponse = response as? HTTPURLResponse else {
                activationError = "Invalid server response"
                return
            }

            let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]

            if httpResponse.statusCode == 200, let jwt = json?["receipt"] as? String {
                let receipt = ActivationReceipt(
                    jwt: jwt,
                    lastSeenTime: Date().timeIntervalSince1970,
                    activatedAt: Date().timeIntervalSince1970
                )
                saveReceipt(receipt)
                loadAndValidateReceipt()
            } else {
                activationError = json?["error"] as? String ?? "Activation failed"
            }
        } catch {
            activationError = "Could not reach activation server. Check your internet connection."
        }
    }

    // MARK: - Deactivation

    func deactivate() {
        KeychainHelper.delete()
        status = .expired
    }

    // MARK: - Internal

    private func loadAndValidateReceipt() {
        guard let data = KeychainHelper.load(),
              let receipt = try? JSONDecoder().decode(ActivationReceipt.self, from: data) else {
            status = .expired
            return
        }

        // Validate JWT signature
        guard let publicKey = activationPublicKey,
              let claims = JWTValidator.validate(receipt.jwt, publicKey: publicKey) else {
            // Also try trial key
            if let trialKey = trialPublicKey,
               let claims = JWTValidator.validate(receipt.jwt, publicKey: trialKey) {
                validateClaims(claims, receipt: receipt, isTrial: true)
                return
            }
            status = .expired
            return
        }

        validateClaims(claims, receipt: receipt, isTrial: claims.type == "trial")
    }

    private func validateClaims(_ claims: JWTValidator.Claims, receipt: ActivationReceipt, isTrial: Bool) {
        let now = Date().timeIntervalSince1970

        // Clock rollback detection (trial only)
        if isTrial && now < receipt.lastSeenTime - 60 {
            // Clock was rolled back more than 60 seconds — expire trial
            status = .expired
            return
        }

        // Update last seen time
        var updated = receipt
        updated.lastSeenTime = now
        saveReceipt(updated)

        if isTrial {
            let remaining = Int(ceil((claims.exp - now) / 86400))
            if remaining > 0 {
                status = .trial(daysRemaining: remaining)
            } else {
                status = .expired
            }
        } else {
            // Paid activation — no expiry, works forever
            status = .activated
        }
    }

    private func saveReceipt(_ receipt: ActivationReceipt) {
        guard let data = try? JSONEncoder().encode(receipt) else { return }
        KeychainHelper.save(data)
    }

    private var activationPublicKey: Curve25519.Signing.PublicKey? {
        guard let keyString = BuildChannel.activationPublicKey,
              let keyData = JWTValidator.base64URLDecode(keyString) else { return nil }
        return try? Curve25519.Signing.PublicKey(rawRepresentation: keyData)
    }

    private var trialPublicKey: Curve25519.Signing.PublicKey? {
        guard let keyString = BuildChannel.trialSigningKey else { return nil }
        // Derive public key from trial private key
        guard let keyData = JWTValidator.base64URLDecode(keyString),
              let privateKey = try? Curve25519.Signing.PrivateKey(rawRepresentation: keyData) else { return nil }
        return privateKey.publicKey
    }

    // Expose base64URL decode for key parsing
    private static func base64URLDecode(_ string: String) -> Data? {
        JWTValidator.base64URLDecode(string)
    }
}

// MARK: - Receipt Storage Model

private struct ActivationReceipt: Codable {
    var jwt: String
    var lastSeenTime: TimeInterval
    var activatedAt: TimeInterval
}
