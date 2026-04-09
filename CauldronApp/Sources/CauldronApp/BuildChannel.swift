import Foundation

/// Build channel detection.
///
/// Official releases are built with `-DCAULDRON_OFFICIAL` in the Swift compiler flags.
/// Self-builds (the default) don't set this flag, so `isOfficialBuild` is false.
///
/// This controls:
/// - Auto-update checking (official only)
/// - Activation/trial gating (official only)
///
/// This is not DRM. Community builds are fully functional with zero restrictions.
/// Official builds require a one-time activation (or 14-day trial) because we
/// need to pay for signing, notarization, hosting, and development.
enum BuildChannel {
    #if CAULDRON_OFFICIAL
    static let isOfficialBuild = true
    #else
    static let isOfficialBuild = false
    #endif

    static var displayName: String {
        isOfficialBuild ? "Official" : "Community Build"
    }

    /// Whether this build requires activation to launch games.
    /// Community builds: false (no activation, no trial, no nag).
    /// Official builds: true (14-day trial, then activation required).
    static var requiresActivation: Bool {
        isOfficialBuild
    }

    /// Ed25519 public key for verifying activation receipts from the server.
    /// The corresponding private key lives only on the activation API server.
    /// Community builds don't use this — it's nil and all checks are skipped.
    #if CAULDRON_OFFICIAL
    static let activationPublicKey: String? = "REPLACE_WITH_REAL_PUBLIC_KEY_BASE64URL"
    #else
    static let activationPublicKey: String? = nil
    #endif

    /// Ed25519 private key for signing trial receipts locally.
    /// This is a DIFFERENT key from the activation key. Trial receipts are
    /// self-signed by the app. A determined attacker could extract this from
    /// the binary — that's acceptable. The goal is to stop casual tampering,
    /// not to be uncrackable. If someone goes to that effort, they could also
    /// just build from source.
    #if CAULDRON_OFFICIAL
    static let trialSigningKey: String? = "REPLACE_WITH_TRIAL_PRIVATE_KEY_BASE64URL"
    #else
    static let trialSigningKey: String? = nil
    #endif
}
