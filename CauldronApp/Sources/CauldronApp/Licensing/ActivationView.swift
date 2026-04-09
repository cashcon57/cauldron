import SwiftUI

/// Activation sheet shown on official builds when trial has expired
/// or on first launch (offering to start trial or enter code).
struct ActivationView: View {
    @Environment(LicenseManager.self) private var licenseManager
    @State private var activationCode: String = ""
    @State private var isActivating: Bool = false

    var body: some View {
        VStack(spacing: 24) {
            // Icon + title
            VStack(spacing: 12) {
                if let url = Bundle.module.url(forResource: "AppIcon", withExtension: "png"),
                   let image = NSImage(contentsOf: url) {
                    Image(nsImage: image)
                        .resizable()
                        .frame(width: 80, height: 80)
                        .clipShape(RoundedRectangle(cornerRadius: 18))
                }

                Text("Welcome to Cauldron")
                    .font(.title2.bold())

                Text("Run Windows games on macOS with bleeding-edge Wine, 131 patches from 9 sources, and one-click graphics backend switching.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 360)
            }

            Divider().padding(.horizontal, 40)

            // Activation code entry
            VStack(spacing: 12) {
                Text("Enter Activation Code")
                    .font(.subheadline.weight(.medium))

                HStack(spacing: 8) {
                    TextField("K7X9M2", text: $activationCode)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(.title3, design: .monospaced))
                        .multilineTextAlignment(.center)
                        .frame(width: 160)
                        .onChange(of: activationCode) { _, newValue in
                            activationCode = String(newValue.uppercased().prefix(6))
                        }

                    Button {
                        isActivating = true
                        Task {
                            await licenseManager.activate(code: activationCode)
                            isActivating = false
                        }
                    } label: {
                        if isActivating {
                            ProgressView().controlSize(.small)
                                .frame(width: 70)
                        } else {
                            Text("Activate")
                                .frame(width: 70)
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(activationCode.count != 6 || isActivating)
                }

                if let error = licenseManager.activationError {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }

            // Or start trial
            if licenseManager.status == .expired || licenseManager.status == .trial(daysRemaining: 0) {
                VStack(spacing: 8) {
                    Text("or")
                        .font(.caption)
                        .foregroundStyle(.tertiary)

                    Button {
                        licenseManager.startTrial()
                    } label: {
                        Text("Start 14-Day Free Trial")
                            .frame(maxWidth: 200)
                    }
                    .buttonStyle(.bordered)

                    Text("Full functionality, no credit card required.")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }
            }

            Spacer().frame(height: 8)

            // Buy link
            HStack(spacing: 4) {
                Text("Don't have a code?")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Link("Buy Cauldron", destination: URL(string: "https://cauldron.app/buy")!)
                    .font(.caption.weight(.medium))
            }
        }
        .padding(32)
        .frame(width: 440, height: 480)
    }
}
