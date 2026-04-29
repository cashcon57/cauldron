import SwiftUI

struct SettingsView: View {
    var body: some View {
        TabView {
            ProfileSettingsTab()
                .tabItem {
                    Label("Profile", systemImage: "flame")
                }

            GeneralSettingsTab()
                .tabItem {
                    Label("General", systemImage: "gearshape")
                }

            AdvancedSettingsTab()
                .tabItem {
                    Label("Advanced", systemImage: "slider.horizontal.3")
                }

            SyncSettingsTab()
                .tabItem {
                    Label("Sync", systemImage: "arrow.triangle.2.circlepath")
                }

            PerformanceSettingsTab()
                .tabItem {
                    Label("Performance", systemImage: "gauge.with.dots.needle.33percent")
                }

            AboutSettingsTab()
                .tabItem {
                    Label("About", systemImage: "info.circle")
                }
        }
        .frame(width: 600, height: 540)
    }
}

// MARK: - Profile Tab (Hero)

private struct ProfileSettingsTab: View {
    @State private var settings = AppSettings.shared
    @State private var showApplyConfirmation = false
    @State private var pendingProfile: ConfigProfile? = nil

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                // Hero profile selector
                VStack(spacing: 12) {
                    Text("Optimization Profile")
                        .font(.title3.weight(.bold))

                    Text("Choose how aggressively Cauldron optimizes. This sets all graphics, sync, and performance options at once. You can fine-tune individual settings in the Advanced tab.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 460)
                }

                HStack(spacing: 12) {
                    ForEach(ConfigProfile.allCases, id: \.self) { profile in
                        ProfileCard(
                            profile: profile,
                            isActive: settings.activeProfile == profile,
                            onSelect: {
                                if profile == settings.activeProfile { return }
                                pendingProfile = profile
                                showApplyConfirmation = true
                            }
                        )
                    }
                }

                // Drift warning
                if !settings.globalMatchesProfile {
                    HStack(spacing: 8) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundStyle(.orange)
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Settings have been customized")
                                .font(.subheadline.weight(.medium))
                            Text("Some advanced settings differ from the \(settings.activeProfile.displayName) profile defaults. Switch profile to reset, or keep your custom configuration.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("Reset to \(settings.activeProfile.displayName)") {
                            withAnimation { settings.applyProfile(settings.activeProfile) }
                        }
                        .font(.caption)
                        .buttonStyle(.plain)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .glassEffect(.regular.tint(.orange).interactive(), in: .capsule)
                    }
                    .padding(12)
                    .glassEffect(.regular, in: .rect(cornerRadius: 10))
                }

                // Current profile summary
                settingsCard("Active: \(settings.activeProfile.displayName)") {
                    let preset = settings.activeProfile.preset
                    ProfileSummaryRow("Graphics Backend", value: preset.graphicsBackend.displayName)
                    ProfileSummaryRow("RosettaX87", value: preset.rosettaX87Enabled ? "Enabled" : "Disabled")
                    ProfileSummaryRow("Async Shaders", value: preset.asyncShaderCompilation ? "Enabled" : "Disabled")
                    ProfileSummaryRow("MetalFX Upscaling", value: preset.metalFXSpatialUpscaling ? "Enabled" : "Disabled")
                    ProfileSummaryRow("DXR Ray Tracing", value: preset.dxrRayTracing ? "Enabled" : "Disabled")
                    ProfileSummaryRow("Auto-Apply Game Patches", value: preset.autoApplyGamePatches ? "Enabled" : "Disabled")
                    ProfileSummaryRow("Nightly Patches", value: preset.showNightlyPatches ? "Shown" : "Hidden")
                    ProfileSummaryRow("Sync Interval", value: preset.syncInterval.displayName)
                    ProfileSummaryRow("Performance Monitoring", value: preset.enablePerformanceMonitoring ? "Enabled" : "Disabled")
                }
            }
            .padding(20)
        }
        .alert("Switch Profile", isPresented: $showApplyConfirmation) {
            Button("Cancel", role: .cancel) { pendingProfile = nil }
            Button("Apply \(pendingProfile?.displayName ?? "")") {
                if let profile = pendingProfile {
                    withAnimation { settings.applyProfile(profile) }
                }
                pendingProfile = nil
            }
        } message: {
            Text("This will reset all settings to the \(pendingProfile?.displayName ?? "") defaults. Per-game overrides will not be affected.")
        }
    }
}

private struct ProfileCard: View {
    let profile: ConfigProfile
    let isActive: Bool
    let onSelect: () -> Void

    var body: some View {
        Button(action: onSelect) {
            VStack(spacing: 10) {
                Image(systemName: profile.icon)
                    .font(.system(size: 28))
                    .foregroundStyle(isActive ? profile.tintColor : .secondary)

                Text(profile.displayName)
                    .font(.headline)
                    .foregroundStyle(.primary)

                Text(profile.tagline)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .lineLimit(3)
                    .frame(height: 36)

                if isActive {
                    Text("Active")
                        .font(.caption2.weight(.bold))
                        .foregroundStyle(profile.tintColor)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 2)
                        .background(profile.tintColor.opacity(0.15))
                        .clipShape(Capsule())
                } else {
                    Text("Select")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 2)
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 14)
            .padding(.horizontal, 8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .glassEffect(
            isActive ? .regular.tint(profile.tintColor) : .regular,
            in: .rect(cornerRadius: 14)
        )
    }
}

private struct ProfileSummaryRow: View {
    let label: String
    let value: String

    init(_ label: String, value: String) {
        self.label = label
        self.value = value
    }

    var body: some View {
        HStack {
            Text(label)
                .foregroundStyle(.secondary)
                .font(.subheadline)
            Spacer()
            Text(value)
                .font(.subheadline)
        }
    }
}

// MARK: - General Tab

private struct GeneralSettingsTab: View {
    @Environment(LicenseManager.self) private var licenseManager: LicenseManager?
    @State private var settings = AppSettings.shared

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                settingsCard("Wine & Bottles") {
                    Picker("Default Wine version", selection: $settings.defaultWineVersion) {
                        ForEach(AppSettings.availableWineVersions, id: \.self) { version in
                            Text(version).tag(version)
                        }
                    }

                    Picker("Default graphics backend", selection: $settings.defaultGraphicsBackend) {
                        ForEach(GraphicsBackend.allCases, id: \.self) { backend in
                            Text(backend.displayName).tag(backend)
                        }
                    }

                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Bottles directory")
                                .font(.body)
                            Text(settings.bottlesDirectory)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                                .truncationMode(.middle)
                        }
                        Spacer()
                        Button("Choose...") {
                            chooseBottlesDirectory()
                        }
                    }
                }

                settingsCard("Behavior") {
                    Toggle("Auto-launch Steam in bottles", isOn: $settings.autoLaunchSteam)
                    if BuildChannel.isOfficialBuild {
                        Toggle("Check for updates automatically", isOn: $settings.checkForUpdates)
                    } else {
                        HStack {
                            Toggle("Check for updates automatically", isOn: .constant(false))
                                .disabled(true)
                            Spacer()
                        }
                        Text("Auto-updates are only available on official builds.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                settingsCard("About") {
                    HStack {
                        Text("Build")
                            .foregroundStyle(.secondary)
                        Spacer()
                        Text(BuildChannel.displayName)
                            .foregroundStyle(BuildChannel.isOfficialBuild ? .primary : .secondary)
                    }
                    if let licenseManager {
                        HStack {
                            Text("License")
                                .foregroundStyle(.secondary)
                            Spacer()
                            Text(licenseManager.status.displayName)
                                .foregroundStyle(licenseStatusColor)
                        }
                        if licenseManager.status == .activated {
                            Button("Deactivate this machine") {
                                licenseManager.deactivate()
                            }
                            .font(.caption)
                            .foregroundStyle(.red)
                        }
                    }
                }
            }
            .padding(20)
        }
    }

    private var licenseStatusColor: Color {
        guard let licenseManager else { return .secondary }
        switch licenseManager.status {
        case .community: return .secondary
        case .trial: return .orange
        case .activated: return .green
        case .expired: return .red
        }
    }

    private func chooseBottlesDirectory() {
        let panel = NSOpenPanel()
        panel.title = "Choose Bottles Directory"
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            settings.bottlesDirectory = url.path
        }
    }
}

// MARK: - Advanced Tab (Power Users)

private struct AdvancedSettingsTab: View {
    @State private var settings = AppSettings.shared
    @State private var rosettaX87Status: CauldronBridge.RosettaX87Status? = nil

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                // Drift indicator
                if !settings.globalMatchesProfile {
                    HStack(spacing: 6) {
                        Image(systemName: "info.circle")
                            .foregroundStyle(.blue)
                        Text("Custom settings active — some values differ from the \(settings.activeProfile.displayName) profile.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .glassEffect(.regular.tint(.blue), in: .rect(cornerRadius: 8))
                }

                settingsCard("Graphics Backend") {
                    Picker("Default backend", selection: $settings.defaultGraphicsBackend) {
                        ForEach(GraphicsBackend.allCases, id: \.self) { backend in
                            Text(backend.displayName).tag(backend)
                        }
                    }
                }

                settingsCard("Display") {
                    VStack(alignment: .leading, spacing: 4) {
                        Toggle("High Resolution Mode (Retina)", isOn: $settings.highResolutionMode)
                        Text("Renders at the full physical pixel resolution on Retina displays. May increase GPU load significantly. Off by default.")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }

                settingsCard("Metal Features") {
                    Toggle("Metal Performance HUD", isOn: $settings.metalPerformanceHUD)
                    Toggle("MetalFX Spatial Upscaling", isOn: $settings.metalFXSpatialUpscaling)

                    VStack(alignment: .leading, spacing: 4) {
                        Toggle("DXR / Ray Tracing (M3+ only)", isOn: $settings.dxrRayTracing)
                        Text("Requires Apple M3 or later. May reduce performance on complex scenes.")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }

                settingsCard("DXVK / MoltenVK") {
                    Toggle("Async shader compilation (DXVK)", isOn: $settings.asyncShaderCompilation)

                    VStack(alignment: .leading, spacing: 4) {
                        Toggle("MoltenVK argument buffers", isOn: $settings.moltenVKArgumentBuffers)
                        Label(
                            "Can cause up to 50% FPS regression in some titles. Leave off unless a specific game needs it.",
                            systemImage: "exclamationmark.triangle.fill"
                        )
                        .font(.caption2)
                        .foregroundStyle(.orange)
                    }
                }

                settingsCard("RosettaX87") {
                    VStack(alignment: .leading, spacing: 8) {
                        Toggle(isOn: $settings.rosettaX87Enabled) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Enable RosettaX87 acceleration")
                                    .font(.body)
                                Text("4-10x faster x87 floating-point. Benefits mod loaders (SKSE, F4SE) and older DX9 games.")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .disabled(!(rosettaX87Status?.available ?? false))

                        if let status = rosettaX87Status {
                            HStack(spacing: 6) {
                                Image(systemName: status.available ? "checkmark.circle.fill" : "xmark.circle")
                                    .foregroundStyle(status.available ? .green : .orange)
                                Text(status.label)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }

                settingsCard("Game Compatibility") {
                    VStack(alignment: .leading, spacing: 4) {
                        Toggle("Auto-apply game binary patches", isOn: $settings.autoApplyGamePatches)
                        Text("Automatically apply known GPU check and driver version fixes when launching games. Original executables are backed up.")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .padding(20)
        }
        .onAppear {
            rosettaX87Status = CauldronBridge.shared.detectRosettaX87()
        }
    }
}

// MARK: - Sync Tab

private struct SyncSettingsTab: View {
    @State private var settings = AppSettings.shared
    @State private var isSyncing = false

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                settingsCard("Auto-Sync") {
                    Toggle("Enable auto-sync", isOn: $settings.enableAutoSync)

                    Picker("Sync interval", selection: $settings.syncInterval) {
                        ForEach(SyncInterval.allCases, id: \.self) { interval in
                            Text(interval.displayName).tag(interval)
                        }
                    }
                    .disabled(!settings.enableAutoSync)
                }

                settingsCard("Repository") {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Proton repository URL")
                            .font(.body)
                        TextField("Repository URL", text: $settings.protonRepositoryURL)
                            .textFieldStyle(.roundedBorder)
                    }

                    Toggle("Show nightly patches", isOn: $settings.showNightlyPatches)
                }

                GlassEffectContainer {
                    HStack {
                        Spacer()
                        Button {
                            withAnimation { isSyncing = true }
                            let bridge = CauldronBridge.shared
                            Task.detached {
                                let _ = bridge.runSync()
                                await MainActor.run {
                                    withAnimation { isSyncing = false }
                                }
                            }
                        } label: {
                            HStack(spacing: 8) {
                                if isSyncing {
                                    ProgressView()
                                        .controlSize(.small)
                                } else {
                                    Image(systemName: "arrow.clockwise")
                                }
                                Text(isSyncing ? "Syncing..." : "Sync Now")
                            }
                            .padding(.horizontal, 20)
                            .padding(.vertical, 10)
                        }
                        .buttonStyle(.plain)
                        .glassEffect(.regular.tint(.accentColor).interactive(), in: .capsule)
                        .disabled(isSyncing)
                        Spacer()
                    }
                    .padding(.vertical, 8)
                }
            }
            .padding(20)
        }
    }
}

// MARK: - Performance Tab

private struct PerformanceSettingsTab: View {
    @State private var settings = AppSettings.shared
    @State private var cacheSize: String = "Calculating..."
    @State private var showClearLogsConfirmation = false
    @State private var showClearCacheConfirmation = false

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                settingsCard("Monitoring") {
                    Toggle("Enable performance monitoring", isOn: $settings.enablePerformanceMonitoring)
                    Toggle("Frame timing overlay", isOn: $settings.frameTimingOverlay)
                        .disabled(!settings.enablePerformanceMonitoring)
                }

                settingsCard("Logging") {
                    Picker("Log level", selection: $settings.logLevel) {
                        ForEach(LogLevel.allCases, id: \.self) { level in
                            Text(level.displayName).tag(level)
                        }
                    }

                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Log directory")
                                .font(.body)
                            Text(settings.logDirectory)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                                .truncationMode(.middle)
                        }
                        Spacer()
                        Button("Open") {
                            NSWorkspace.shared.open(URL(fileURLWithPath: settings.logDirectory))
                        }
                    }

                    HStack {
                        Spacer()
                        Button("Clear All Logs") {
                            showClearLogsConfirmation = true
                        }
                        .foregroundStyle(.red)
                    }
                }

                settingsCard("Shader Cache") {
                    HStack {
                        Text("Total cache size")
                        Spacer()
                        Text(cacheSize)
                            .foregroundStyle(.secondary)
                    }

                    HStack {
                        Spacer()
                        Button("Clear All Caches") {
                            showClearCacheConfirmation = true
                        }
                        .foregroundStyle(.red)
                    }
                }
            }
            .padding(20)
        }
        .onAppear { calculateCacheSize() }
        .alert("Clear All Logs", isPresented: $showClearLogsConfirmation) {
            Button("Cancel", role: .cancel) {}
            Button("Clear", role: .destructive) { clearLogs() }
        } message: {
            Text("This will permanently delete all log files. This action cannot be undone.")
        }
        .alert("Clear All Caches", isPresented: $showClearCacheConfirmation) {
            Button("Cancel", role: .cancel) {}
            Button("Clear", role: .destructive) { clearCaches() }
        } message: {
            Text("This will delete all compiled shader caches. Games may stutter briefly while shaders are recompiled.")
        }
    }

    private func calculateCacheSize() {
        let cachePath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Caches/Cauldron/Shaders")
        if let enumerator = FileManager.default.enumerator(at: cachePath, includingPropertiesForKeys: [.fileSizeKey]) {
            var total: Int64 = 0
            for case let fileURL as URL in enumerator {
                if let size = try? fileURL.resourceValues(forKeys: [.fileSizeKey]).fileSize {
                    total += Int64(size)
                }
            }
            cacheSize = ByteCountFormatter.string(fromByteCount: total, countStyle: .file)
        } else {
            cacheSize = "0 bytes"
        }
    }

    private func clearLogs() {
        let logPath = URL(fileURLWithPath: settings.logDirectory)
        try? FileManager.default.removeItem(at: logPath)
        try? FileManager.default.createDirectory(at: logPath, withIntermediateDirectories: true)
    }

    private func clearCaches() {
        let cachePath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Caches/Cauldron/Shaders")
        try? FileManager.default.removeItem(at: cachePath)
        try? FileManager.default.createDirectory(at: cachePath, withIntermediateDirectories: true)
        calculateCacheSize()
    }
}

// MARK: - About Tab

private struct AboutSettingsTab: View {
    private let appVersion = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.1.0"
    private let buildNumber = Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "1"

    var body: some View {
        VStack(spacing: 20) {
            Spacer()

            VStack(spacing: 12) {
                Image(systemName: "flame.fill")
                    .font(.system(size: 56))
                    .foregroundStyle(.orange.gradient)

                Text("Cauldron")
                    .font(.title.weight(.bold))

                Text("Version \(appVersion) (\(buildNumber))")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Text("A macOS game compatibility layer that translates Windows gaming APIs to native Apple Metal, powered by Wine and community-driven Proton patches.")
                .font(.body)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
                .frame(maxWidth: 400)
                .padding(.horizontal, 20)
                .padding(.vertical, 14)
                .glassEffect(.regular, in: .rect(cornerRadius: 12))

            Text("Licensed under LGPL-2.1")
                .font(.caption)
                .foregroundStyle(.secondary)

            GlassEffectContainer {
                HStack(spacing: 12) {
                    linkButton("GitHub", icon: "chevron.left.forwardslash.chevron.right",
                               url: "https://github.com/cauldron-app/cauldron")
                    linkButton("Report Issue", icon: "exclamationmark.bubble",
                               url: "https://github.com/cauldron-app/cauldron/issues/new")
                    linkButton("Documentation", icon: "book",
                               url: "https://github.com/cauldron-app/cauldron/wiki")
                }
                .padding(8)
            }

            Spacer()
        }
        .frame(maxWidth: .infinity)
    }

    private func linkButton(_ title: String, icon: String, url: String) -> some View {
        Button {
            if let link = URL(string: url) {
                NSWorkspace.shared.open(link)
            }
        } label: {
            Label(title, systemImage: icon)
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
        }
        .buttonStyle(.plain)
        .glassEffect(.regular.tint(.accentColor).interactive(), in: .capsule)
    }
}

// MARK: - Reusable Settings Card

private func settingsCard<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
    VStack(alignment: .leading, spacing: 12) {
        Text(title)
            .font(.headline)

        VStack(alignment: .leading, spacing: 10) {
            content()
        }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(16)
    .glassEffect(.regular, in: .rect(cornerRadius: 12))
}
