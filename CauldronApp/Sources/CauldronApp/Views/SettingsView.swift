import SwiftUI

struct SettingsView: View {
    var body: some View {
        TabView {
            GeneralSettingsTab()
                .tabItem {
                    Label("General", systemImage: "gearshape")
                }

            GraphicsSettingsTab()
                .tabItem {
                    Label("Graphics", systemImage: "rectangle.stack.badge.play")
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
        .frame(width: 560, height: 480)
    }
}

// MARK: - General Tab

private struct GeneralSettingsTab: View {
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
                    Toggle("Check for updates automatically", isOn: $settings.checkForUpdates)
                }
            }
            .padding(20)
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

// MARK: - Graphics Tab

private struct GraphicsSettingsTab: View {
    @State private var settings = AppSettings.shared

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                settingsCard("Translation Backend") {
                    Picker("Default backend", selection: $settings.defaultGraphicsBackend) {
                        ForEach(GraphicsBackend.allCases, id: \.self) { backend in
                            Text(backend.displayName).tag(backend)
                        }
                    }
                }

                settingsCard("Metal") {
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
                            "Enabling argument buffers can cause performance regressions in some titles.",
                            systemImage: "exclamationmark.triangle.fill"
                        )
                        .font(.caption2)
                        .foregroundStyle(.orange)
                    }
                }
            }
            .padding(20)
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
