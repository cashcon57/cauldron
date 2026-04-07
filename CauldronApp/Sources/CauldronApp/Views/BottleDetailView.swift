import SwiftUI
import UniformTypeIdentifiers

struct BottleDetailView: View {
    let bottle: Bottle

    @State private var graphicsBackend: GraphicsBackend = .auto
    @State private var msyncEnabled: Bool = true
    @State private var esyncEnabled: Bool = true
    @State private var fsrEnabled: Bool = false
    @State private var largeAddressAware: Bool = false
    @State private var showLaunchError: Bool = false
    @State private var launchErrorMessage: String = ""
    @State private var detectedGames: [GameRecord] = []
    @State private var isScanning: Bool = false
    @State private var showOtherExes: Bool = false

    var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                bottleInfoCard
                detectedGamesCard
                graphicsCard
                syncCard
                gameFixesCard
                actionsCard
            }
            .padding(24)
        }
        .navigationTitle(bottle.name)
        .onAppear {
            loadPerBottleSettings()
            scanForGames()
        }
        .onChange(of: graphicsBackend) { _, newValue in
            saveSettingForBottle("graphicsBackend", value: newValue.rawValue)
        }
        .onChange(of: msyncEnabled) { _, newValue in
            saveSettingForBottle("msyncEnabled", value: newValue)
        }
        .onChange(of: esyncEnabled) { _, newValue in
            saveSettingForBottle("esyncEnabled", value: newValue)
        }
        .onChange(of: fsrEnabled) { _, newValue in
            saveSettingForBottle("fsrEnabled", value: newValue)
        }
        .onChange(of: largeAddressAware) { _, newValue in
            saveSettingForBottle("largeAddressAware", value: newValue)
        }
        .alert("Launch Error", isPresented: $showLaunchError) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(launchErrorMessage)
        }
    }

    private var bottleInfoCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Bottle Info", systemImage: "info.circle")
                .font(.headline)
                .foregroundStyle(.primary)

            VStack(spacing: 8) {
                DetailRow(label: "Name", value: bottle.name)
                DetailRow(label: "Wine Version", value: bottle.wineVersion)
                DetailRow(label: "Created", value: formattedDate)
                DetailRow(label: "Path", value: bottle.path, isCaption: true)
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 16))
    }

    private var graphicsCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Graphics", systemImage: "gpu")
                .font(.headline)

            Text("Choose how DirectX calls are translated to Metal on macOS.")
                .font(.caption)
                .foregroundStyle(.secondary)

            ForEach(GraphicsBackend.allCases, id: \.self) { backend in
                Button {
                    graphicsBackend = backend
                } label: {
                    HStack(spacing: 10) {
                        Image(systemName: graphicsBackend == backend ? "circle.inset.filled" : "circle")
                            .foregroundStyle(graphicsBackend == backend ? .blue : .secondary)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(backend.displayName)
                                .font(.subheadline.weight(.medium))
                                .foregroundStyle(.primary)
                            Text(backendDescription(backend))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                        }
                        Spacer()
                    }
                    .padding(.vertical, 3)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 16))
    }

    private func backendDescription(_ backend: GraphicsBackend) -> String {
        switch backend {
        case .d3dMetal: return "Apple's Game Porting Toolkit. Best for DX11/DX12 on Apple Silicon."
        case .dxmt: return "Community DX11-to-Metal translator. Often faster than D3DMetal for DX11."
        case .dxvkMoltenVK: return "DX9/10/11 → Vulkan → Metal. Most compatible for older games."
        case .dxvkKosmicKrisp: return "Mesa Vulkan 1.3 on Metal 4. Best DX9-11 performance on macOS 26+."
        case .vkd3dProton: return "DX12-to-Vulkan. Required for DX12-only games."
        case .auto: return "Let Cauldron pick the best backend based on each game's needs."
        }
    }

    private var syncCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Synchronization", systemImage: "arrow.triangle.2.circlepath")
                .font(.headline)

            Text("Controls how Wine synchronizes threads. Affects game stability and performance.")
                .font(.caption)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 8) {
                Toggle(isOn: $msyncEnabled) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("MSync (Mach semaphore sync)")
                            .font(.subheadline)
                        Text("Uses macOS-native semaphores for fast thread synchronization. Recommended — leave enabled for best performance.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                Toggle(isOn: $esyncEnabled) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("ESync (fallback)")
                            .font(.subheadline)
                        Text("Event-based synchronization fallback. Used when MSync isn't available or a game has issues with it. Safe to leave enabled.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 16))
    }

    private var gameFixesCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Game Fixes", systemImage: "wrench.and.screwdriver")
                .font(.headline)

            Text("Optional fixes for specific compatibility issues. Only enable if a game needs it.")
                .font(.caption)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 8) {
                Toggle(isOn: $fsrEnabled) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("AMD FidelityFX Super Resolution (FSR)")
                            .font(.subheadline)
                        Text("Upscales lower-resolution rendering to your display resolution. Improves performance at the cost of some visual quality. Enable if a game runs too slowly at native resolution.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                Toggle(isOn: $largeAddressAware) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Large Address Aware")
                            .font(.subheadline)
                        Text("Allows 32-bit games to use more than 2GB of RAM. Enable for heavily modded games (Skyrim, Fallout) that crash with out-of-memory errors. Leave off for most games.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 16))
    }

    private var actionsCard: some View {
        GlassEffectContainer(spacing: 12) {
            HStack(spacing: 12) {
                Button {
                    selectAndLaunchExe()
                } label: {
                    Label("Launch...", systemImage: "play.fill")
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 8)
                }
                .buttonStyle(.plain)
                .glassEffect(.regular.tint(.green).interactive(), in: .capsule)

                Button {
                    openBottleFolder()
                } label: {
                    Label("Open Folder", systemImage: "folder")
                        .padding(.vertical, 8)
                        .padding(.horizontal, 12)
                }
                .buttonStyle(.plain)
                .glassEffect(.regular.interactive(), in: .capsule)
            }
        }
    }

    private var games: [GameRecord] {
        detectedGames.filter { $0.knownIssues == "game" }
    }

    private var utilities: [GameRecord] {
        detectedGames.filter { $0.knownIssues != "game" }
    }

    private var detectedGamesCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("Detected Games", systemImage: "gamecontroller")
                    .font(.headline)
                Spacer()
                if isScanning {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Button {
                        scanForGames()
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                    .buttonStyle(.plain)
                    .help("Rescan for games")
                }
            }

            if detectedGames.isEmpty && !isScanning {
                Text("No games detected in this bottle.")
                    .foregroundStyle(.secondary)
                    .font(.subheadline)
            } else {
                // Games at top
                if !games.isEmpty {
                    ForEach(games, id: \.id) { game in
                        gameRow(game)
                    }
                } else if !isScanning {
                    Text("No games found — try installing a game via Steam first.")
                        .foregroundStyle(.secondary)
                        .font(.subheadline)
                }

                // Other executables in a collapsed section
                if !utilities.isEmpty {
                    Divider()
                        .padding(.vertical, 4)

                    DisclosureGroup(isExpanded: $showOtherExes) {
                        ForEach(utilities, id: \.id) { exe in
                            gameRow(exe)
                        }
                    } label: {
                        Text("\(utilities.count) other executable\(utilities.count == 1 ? "" : "s")")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 16))
    }

    private func gameRow(_ game: GameRecord) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(game.title)
                    .font(.body.weight(.medium))
                if !game.notes.isEmpty {
                    Text(game.notes)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
            Spacer()
            // Only show badge when there's a meaningful status
            if !game.compatStatus.isEmpty && game.compatStatus.lowercased() != "unknown" {
                Text(game.compatStatus.capitalized)
                    .font(.caption)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 2)
                    .background(statusColor(for: game.compatStatus).opacity(0.2))
                    .foregroundStyle(statusColor(for: game.compatStatus))
                    .clipShape(Capsule())
            }
        }
        .padding(.vertical, 4)
    }

    private func statusColor(for status: String) -> Color {
        switch status.lowercased() {
        case "playable": return .green
        case "runs": return .yellow
        case "unrated": return .secondary
        default: return .secondary
        }
    }

    private func scanForGames() {
        isScanning = true
        let bottleId = bottle.id
        Task {
            let games = CauldronBridge.shared.scanBottleGames(bottleId: bottleId)
            detectedGames = games
            isScanning = false
        }
    }

    private var formattedDate: String {
        let formatter = ISO8601DateFormatter()
        if let date = formatter.date(from: bottle.createdAt) {
            return date.formatted(date: .abbreviated, time: .shortened)
        }
        return bottle.createdAt
    }

    // MARK: - Settings Persistence

    private func settingsKey(_ setting: String) -> String {
        "bottle_\(bottle.id)_\(setting)"
    }

    private func loadPerBottleSettings() {
        let defaults = UserDefaults.standard

        if let raw = defaults.string(forKey: settingsKey("graphicsBackend")),
           let backend = GraphicsBackend(rawValue: raw) {
            graphicsBackend = backend
        } else {
            graphicsBackend = GraphicsBackend(rawValue: bottle.graphicsBackend) ?? .auto
        }

        if defaults.object(forKey: settingsKey("msyncEnabled")) != nil {
            msyncEnabled = defaults.bool(forKey: settingsKey("msyncEnabled"))
        } else {
            msyncEnabled = true
        }

        if defaults.object(forKey: settingsKey("esyncEnabled")) != nil {
            esyncEnabled = defaults.bool(forKey: settingsKey("esyncEnabled"))
        } else {
            esyncEnabled = true
        }

        if defaults.object(forKey: settingsKey("fsrEnabled")) != nil {
            fsrEnabled = defaults.bool(forKey: settingsKey("fsrEnabled"))
        } else {
            fsrEnabled = false
        }

        if defaults.object(forKey: settingsKey("largeAddressAware")) != nil {
            largeAddressAware = defaults.bool(forKey: settingsKey("largeAddressAware"))
        } else {
            largeAddressAware = false
        }
    }

    private func saveSettingForBottle(_ key: String, value: Any) {
        UserDefaults.standard.set(value, forKey: settingsKey(key))
    }

    // MARK: - Actions

    private func selectAndLaunchExe() {
        let panel = NSOpenPanel()
        panel.title = "Select a Windows executable"
        panel.allowedContentTypes = [UTType(filenameExtension: "exe")].compactMap { $0 }
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false

        if panel.runModal() == .OK, let url = panel.url {
            // Check if Wine is available by looking at known versions
            let wineVersions = CauldronBridge.shared.getWineVersions()
            if wineVersions.isEmpty {
                // Also check filesystem for wine binaries
                let home = FileManager.default.homeDirectoryForCurrentUser.path
                let searchPaths = [
                    "\(home)/Library/Cauldron/wine/bin/wine64",
                    "/usr/local/bin/wine64",
                    "/usr/local/bin/wine",
                    "/opt/homebrew/bin/wine64",
                    "/opt/homebrew/bin/wine",
                ]
                let hasWine = searchPaths.contains { FileManager.default.fileExists(atPath: $0) }
                if !hasWine {
                    launchErrorMessage = "Wine not installed. Go to Settings or run `cauldron wine install` from the command line to install Wine first."
                    showLaunchError = true
                    return
                }
            }

            let success = CauldronBridge.shared.launchExe(
                bottleId: bottle.id,
                exePath: url.path,
                backend: graphicsBackend.rawValue
            )

            if !success {
                launchErrorMessage = "Failed to launch \(url.lastPathComponent). Check that Wine is installed and the bottle path exists."
                showLaunchError = true
            }
        }
    }

    private func openBottleFolder() {
        let url = URL(fileURLWithPath: bottle.path, isDirectory: true)
        NSWorkspace.shared.open(url)
    }
}

private struct DetailRow: View {
    let label: String
    let value: String
    var isCaption: Bool = false

    var body: some View {
        HStack {
            Text(label)
                .foregroundStyle(.secondary)
                .frame(width: 100, alignment: .trailing)
            Text(value)
                .font(isCaption ? .caption : .body)
                .foregroundStyle(isCaption ? .secondary : .primary)
                .textSelection(.enabled)
            Spacer()
        }
    }
}
