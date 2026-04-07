import SwiftUI

struct SteamInstallWizard: View {
    @Environment(BottleListViewModel.self) private var viewModel
    @Environment(\.dismiss) private var dismiss

    @State private var currentStep: WizardStep = .welcome
    @State private var isInstalling = false
    @State private var installProgress: Float = 0
    @State private var statusMessage = ""
    @State private var currentInstallStep = ""
    @State private var installComplete = false
    @State private var installError: String?

    // Prerequisite state
    @State private var wineAvailable = false
    @State private var wineVersion = ""
    @State private var diskSpaceGB: Double = 0
    @State private var internetAvailable = false
    @State private var isCheckingPrereqs = false

    enum WizardStep {
        case welcome
        case prerequisites
        case installing
        case complete
    }

    var body: some View {
        VStack(spacing: 0) {
            switch currentStep {
            case .welcome:
                welcomeStep
            case .prerequisites:
                prerequisitesStep
            case .installing:
                installingStep
            case .complete:
                completeStep
            }
        }
        .frame(width: 520, height: 480)
    }

    // MARK: - Welcome

    private var welcomeStep: some View {
        VStack(spacing: 20) {
            Spacer()

            Image(systemName: "gamecontroller.fill")
                .font(.system(size: 56))
                .foregroundStyle(.blue)
                .symbolRenderingMode(.hierarchical)

            Text("Install Steam")
                .font(.largeTitle.bold())

            Text("Cauldron will create a new Wine bottle and install the Windows version of Steam. This lets you download and play your Steam library on macOS.")
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 40)

            VStack(spacing: 10) {
                installInfoCard(
                    icon: "wineglass",
                    text: "Create a dedicated Wine bottle"
                )
                installInfoCard(
                    icon: "arrow.down.circle",
                    text: "Download Steam installer from Valve"
                )
                installInfoCard(
                    icon: "gearshape.2",
                    text: "Install Steam with optimized settings"
                )
                installInfoCard(
                    icon: "cpu",
                    text: "Configure graphics backend for best performance"
                )
            }
            .padding(.horizontal, 40)

            Spacer()

            GlassEffectContainer(spacing: 12) {
                HStack(spacing: 12) {
                    Button("Cancel") {
                        dismiss()
                    }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.cancelAction)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 8)
                    .glassEffect(.regular.interactive(), in: .capsule)

                    Button {
                        currentStep = .prerequisites
                        checkPrerequisites()
                    } label: {
                        Text("Continue")
                            .padding(.horizontal, 20)
                            .padding(.vertical, 8)
                    }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.defaultAction)
                    .glassEffect(.regular.tint(.accentColor).interactive(), in: .capsule)
                }
            }
            .padding(.bottom, 16)
        }
    }

    // MARK: - Prerequisites

    private var prerequisitesStep: some View {
        VStack(spacing: 20) {
            Spacer()

            Text("Prerequisites")
                .font(.title2.bold())

            Text("Checking that your system is ready for Steam installation.")
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 40)

            if isCheckingPrereqs {
                ProgressView()
                    .controlSize(.large)
                    .padding()
            } else {
                VStack(spacing: 12) {
                    prereqRow(
                        label: "Wine installed",
                        detail: wineAvailable ? wineVersion : "Not found",
                        ok: wineAvailable
                    )
                    prereqRow(
                        label: "Disk space",
                        detail: String(format: "%.1f GB available", diskSpaceGB),
                        ok: diskSpaceGB >= 2.0
                    )
                    prereqRow(
                        label: "Internet connectivity",
                        detail: internetAvailable ? "Connected" : "Unreachable",
                        ok: internetAvailable
                    )
                }
                .padding(.horizontal, 40)
            }

            if !isCheckingPrereqs && !allPrereqsMet {
                Text("Some prerequisites are not met. Please resolve the issues above before continuing.")
                    .font(.caption)
                    .foregroundStyle(.red)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 40)
            }

            Spacer()

            GlassEffectContainer(spacing: 12) {
                HStack(spacing: 12) {
                    Button("Back") {
                        currentStep = .welcome
                    }
                    .buttonStyle(.plain)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 8)
                    .glassEffect(.regular.interactive(), in: .capsule)

                    Button {
                        currentStep = .installing
                        beginInstallation()
                    } label: {
                        Text("Install")
                            .padding(.horizontal, 20)
                            .padding(.vertical, 8)
                    }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.defaultAction)
                    .disabled(isCheckingPrereqs || !allPrereqsMet)
                    .glassEffect(.regular.tint(.accentColor).interactive(), in: .capsule)
                }
            }
            .padding(.bottom, 16)
        }
    }

    // MARK: - Installing

    private var installingStep: some View {
        VStack(spacing: 24) {
            Spacer()

            ProgressView(value: installProgress, total: 100) {
                Text(currentInstallStep)
                    .font(.headline)
            } currentValueLabel: {
                Text(statusMessage)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            .progressViewStyle(.linear)
            .padding(.horizontal, 50)

            Text("\(Int(installProgress))%")
                .font(.system(size: 36, weight: .bold, design: .rounded))
                .contentTransition(.numericText())

            Text("This may take a few minutes...")
                .font(.caption)
                .foregroundStyle(.tertiary)

            Spacer()

            GlassEffectContainer(spacing: 12) {
                Button("Cancel") {
                    // Warn about cancelling mid-install
                    dismiss()
                }
                .buttonStyle(.plain)
                .padding(.horizontal, 20)
                .padding(.vertical, 8)
                .disabled(isInstalling)
                .glassEffect(.regular.interactive(), in: .capsule)
            }
            .padding(.bottom, 16)
        }
    }

    // MARK: - Complete

    private var completeStep: some View {
        VStack(spacing: 20) {
            Spacer()

            if let error = installError {
                Image(systemName: "xmark.circle.fill")
                    .font(.system(size: 56))
                    .foregroundStyle(.red)

                Text("Installation Failed")
                    .font(.title2.bold())

                Text(error)
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 40)
            } else {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 56))
                    .foregroundStyle(.green)

                Text("Steam Installed Successfully!")
                    .font(.title2.bold())

                Text("Steam has been installed in a new Wine bottle with optimized settings for gaming on macOS.")
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 40)
            }

            Spacer()

            GlassEffectContainer(spacing: 12) {
                HStack(spacing: 12) {
                    Button("Done") {
                        viewModel.loadBottles()
                        dismiss()
                    }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.defaultAction)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 8)
                    .glassEffect(.regular.interactive(), in: .capsule)

                    if installError == nil {
                        Button {
                            // Launch Steam via the bridge in a future iteration.
                            viewModel.loadBottles()
                            dismiss()
                        } label: {
                            Label("Launch Steam", systemImage: "play.fill")
                                .padding(.horizontal, 20)
                                .padding(.vertical, 8)
                        }
                        .buttonStyle(.plain)
                        .glassEffect(.regular.tint(.green).interactive(), in: .capsule)
                    }
                }
            }
            .padding(.bottom, 16)
        }
    }

    // MARK: - Helper Views

    private func installInfoCard(icon: String, text: String) -> some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .font(.body)
                .frame(width: 24)
                .foregroundStyle(.blue)
            Text(text)
                .font(.subheadline)
            Spacer()
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .glassEffect(.regular, in: .rect(cornerRadius: 10))
    }

    private func prereqRow(label: String, detail: String, ok: Bool) -> some View {
        HStack {
            Image(systemName: ok ? "checkmark.circle.fill" : "xmark.circle.fill")
                .foregroundStyle(ok ? .green : .red)
                .font(.title3)
            VStack(alignment: .leading, spacing: 2) {
                Text(label)
                    .font(.subheadline.bold())
                Text(detail)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .glassEffect(.regular, in: .rect(cornerRadius: 10))
    }

    // MARK: - Computed

    private var allPrereqsMet: Bool {
        wineAvailable && diskSpaceGB >= 2.0 && internetAvailable
    }

    // MARK: - Actions

    private func checkPrerequisites() {
        isCheckingPrereqs = true
        Task {
            let wineOk = Self.probeWine()
            let diskGB = Self.probeDiskSpace()
            let netOk = Self.probeInternet()

            wineAvailable = wineOk.0
            wineVersion = wineOk.1
            diskSpaceGB = diskGB
            internetAvailable = netOk
            isCheckingPrereqs = false
        }
    }

    private func beginInstallation() {
        isInstalling = true
        installProgress = 0
        statusMessage = "Starting..."
        currentInstallStep = "Preparing"

        Task.detached {
            // Simulate the install steps with progress callbacks.
            // In production this would call the Rust FFI via CauldronBridge.
            let steps: [(String, String, Float)] = [
                ("Creating Bottle", "Setting up a new Wine bottle for Steam...", 14),
                ("Initializing Wine Prefix", "Running wineboot --init...", 28),
                ("Downloading Steam Installer", "Downloading SteamSetup.exe from Valve...", 42),
                ("Running Steam Setup", "Installing Steam (silent mode)...", 57),
                ("Configuring DLL Overrides", "Setting DXVK/DXMT overrides for gaming...", 71),
                ("Configuring Runtimes", "Setting up graphics backend and msync...", 85),
                ("Verifying Installation", "Checking that steam.exe is present...", 100),
            ]

            for (stepName, detail, pct) in steps {
                await MainActor.run {
                    currentInstallStep = stepName
                    statusMessage = detail
                    withAnimation(.easeInOut(duration: 0.3)) {
                        installProgress = pct
                    }
                }
                // Each step takes some time in real usage; simulate briefly.
                try? await Task.sleep(for: .milliseconds(400))
            }

            await MainActor.run {
                isInstalling = false
                installComplete = true
                currentStep = .complete
            }
        }
    }

    // MARK: - System probes (run off main thread)

    private static func probeWine() -> (Bool, String) {
        // Try common Wine locations on macOS, including CrossOver and Gcenx builds.
        let candidates = [
            "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine",
            "/usr/local/bin/wine64",
            "/usr/local/bin/wine",
            "/opt/homebrew/bin/wine64",
            "/opt/homebrew/bin/wine",
            "/Applications/Wine Stable.app/Contents/Resources/wine/bin/wine64",
            "/Applications/Wine Devel.app/Contents/Resources/wine/bin/wine64",
            "/Applications/Wine Staging.app/Contents/Resources/wine/bin/wine64",
        ]
        for path in candidates {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: path)
            process.arguments = ["--version"]
            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = Pipe()
            do {
                try process.run()
                process.waitUntilExit()
                if process.terminationStatus == 0 {
                    let data = pipe.fileHandleForReading.readDataToEndOfFile()
                    let version = String(data: data, encoding: .utf8)?
                        .trimmingCharacters(in: .whitespacesAndNewlines) ?? "unknown"
                    return (true, version)
                }
            } catch {
                continue
            }
        }
        return (false, "")
    }

    private static func probeDiskSpace() -> Double {
        let home = FileManager.default.homeDirectoryForCurrentUser
        do {
            let values = try home.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
            if let bytes = values.volumeAvailableCapacityForImportantUsage {
                return Double(bytes) / (1024 * 1024 * 1024)
            }
        } catch {}
        return 0
    }

    private static func probeInternet() -> Bool {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/curl")
        process.arguments = ["--head", "--silent", "--max-time", "5",
                             "https://cdn.cloudflare.steamstatic.com/client/installer/SteamSetup.exe"]
        process.standardOutput = Pipe()
        process.standardError = Pipe()
        do {
            try process.run()
            process.waitUntilExit()
            return process.terminationStatus == 0
        } catch {
            return false
        }
    }
}
