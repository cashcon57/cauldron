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
    @State private var d3dMetalInfo: CauldronBridge.D3DMetalInfo? = nil
    @State private var d3dMetalImporting: Bool = false
    @State private var showD3DMetalFilePicker: Bool = false

    var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                bottleInfoCard
                detectedGamesCard
                graphicsCard
                d3dMetalCard
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
            d3dMetalInfo = CauldronBridge.shared.detectD3DMetal()
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

    private var d3dMetalCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("D3DMetal / GPTK", systemImage: "rectangle.3.group")
                .font(.headline)

            if let info = d3dMetalInfo {
                if info.source == "none" {
                    // Not found — guide user to CrossOver
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 6) {
                            Image(systemName: "exclamationmark.triangle")
                                .foregroundStyle(.orange)
                            Text("D3DMetal not found")
                                .font(.subheadline.weight(.medium))
                        }

                        Text("D3DMetal is Apple's proprietary DirectX-to-Metal translator. It provides the best DX11/DX12 performance but cannot be distributed with Cauldron.")
                            .font(.caption)
                            .foregroundStyle(.secondary)

                        Text("Cauldron works best alongside CrossOver. Install CrossOver and D3DMetal will be imported automatically.")
                            .font(.caption)
                            .foregroundStyle(.secondary)

                        HStack(spacing: 12) {
                            Button {
                                NSWorkspace.shared.open(URL(string: "https://www.codeweavers.com/crossover")!)
                            } label: {
                                Label("Get CrossOver", systemImage: "arrow.up.right")
                                    .padding(.horizontal, 12)
                                    .padding(.vertical, 6)
                            }
                            .buttonStyle(.plain)
                            .glassEffect(.regular.tint(.blue).interactive(), in: .capsule)

                            Button {
                                showD3DMetalFilePicker = true
                            } label: {
                                Label("Import Custom", systemImage: "folder")
                                    .padding(.horizontal, 12)
                                    .padding(.vertical, 6)
                            }
                            .buttonStyle(.plain)
                            .glassEffect(.regular.interactive(), in: .capsule)
                        }
                        .padding(.top, 4)
                    }
                } else {
                    // Found or imported
                    HStack(spacing: 8) {
                        Image(systemName: info.source == "imported" ? "checkmark.circle.fill" : "arrow.down.circle.fill")
                            .foregroundStyle(.green)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(info.label)
                                .font(.subheadline.weight(.medium))
                            Text(info.path)
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                                .lineLimit(1)
                                .truncationMode(.middle)
                        }
                        Spacer()
                        if !info.imported {
                            Button {
                                d3dMetalImporting = true
                                let bridge = CauldronBridge.shared
                                Task.detached {
                                    let result = bridge.importD3DMetal()
                                    await MainActor.run {
                                        d3dMetalImporting = false
                                        d3dMetalInfo = CauldronBridge.shared.detectD3DMetal()
                                    }
                                }
                            } label: {
                                if d3dMetalImporting {
                                    ProgressView().controlSize(.small)
                                } else {
                                    Text("Import")
                                        .padding(.horizontal, 12)
                                        .padding(.vertical, 6)
                                }
                            }
                            .buttonStyle(.plain)
                            .disabled(d3dMetalImporting)
                            .glassEffect(.regular.tint(.green).interactive(), in: .capsule)
                        }
                    }
                }
            } else {
                HStack(spacing: 6) {
                    ProgressView().controlSize(.small)
                    Text("Detecting D3DMetal...")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 16))
        .fileImporter(isPresented: $showD3DMetalFilePicker, allowedContentTypes: [.folder]) { result in
            if case .success(let url) = result {
                let frameworkPath = url.path.hasSuffix("D3DMetal.framework")
                    ? url.path
                    : url.appendingPathComponent("D3DMetal.framework").path
                d3dMetalImporting = true
                let bridge = CauldronBridge.shared
                Task.detached {
                    let _ = bridge.importD3DMetal(customPath: frameworkPath)
                    await MainActor.run {
                        d3dMetalImporting = false
                        d3dMetalInfo = CauldronBridge.shared.detectD3DMetal()
                    }
                }
            }
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

    @State private var showDependencyPicker = false
    @State private var installingDependency: String? = nil

    @State private var bottleRunning = false
    @State private var runningCheckTimer: Timer? = nil

    private var actionsCard: some View {
        GlassEffectContainer(spacing: 12) {
            VStack(spacing: 10) {
                // Running state indicator — always visible
                HStack(spacing: 6) {
                    Circle()
                        .fill(bottleRunning ? .green : .gray.opacity(0.3))
                        .frame(width: 8, height: 8)
                    Text(bottleRunning ? "Wine is running" : "Wine is not running")
                        .font(.caption)
                        .foregroundStyle(bottleRunning ? .green : .secondary)
                    Spacer()

                    if bottleRunning {
                        Button {
                            killWine()
                        } label: {
                            Label("Stop Wine", systemImage: "stop.fill")
                                .font(.caption)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 4)
                        }
                        .buttonStyle(.plain)
                        .glassEffect(.regular.tint(.red).interactive(), in: .capsule)
                    }
                }

                HStack(spacing: 12) {
                    Button {
                        selectAndLaunchExe()
                    } label: {
                        Label("Run .exe", systemImage: "play.fill")
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 8)
                    }
                    .buttonStyle(.plain)
                    .disabled(!(licenseManager?.status.canLaunchGames ?? true))
                    .glassEffect(.regular.tint(.green).interactive(), in: .capsule)

                    Button {
                        killWine()
                    } label: {
                        Label("Kill Wine", systemImage: "xmark.circle.fill")
                            .padding(.vertical, 8)
                            .padding(.horizontal, 12)
                    }
                    .buttonStyle(.plain)
                    .glassEffect(.regular.tint(.red).interactive(), in: .capsule)
                    .help("Kill all Wine/wineserver processes for this bottle")

                    Button {
                        showDependencyPicker = true
                    } label: {
                        Label("Install Deps", systemImage: "shippingbox")
                            .padding(.vertical, 8)
                            .padding(.horizontal, 12)
                    }
                    .buttonStyle(.plain)
                    .glassEffect(.regular.tint(.blue).interactive(), in: .capsule)

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
        .onAppear { checkWineRunning(); startRunningCheck() }
        .onDisappear { runningCheckTimer?.invalidate() }
        .sheet(isPresented: $showDependencyPicker) {
            DependencyPickerSheet(bottleId: bottle.id, installingId: $installingDependency)
        }
    }

    /// Kill Wine using both Cauldron's tracker AND wineserver --kill on the bottle path.
    private func killWine() {
        // Kill via Cauldron's PID tracker
        let _ = CauldronBridge.shared.killBottle(bottleId: bottle.id)

        // Also kill any wineserver whose WINEPREFIX matches this bottle.
        // This catches processes launched outside Cauldron (e.g. via CrossOver).
        let bottlePath = bottle.path
        Task.detached {
            // Find wineserver processes and kill them
            let ps = Process()
            ps.executableURL = URL(fileURLWithPath: "/bin/sh")
            ps.arguments = ["-c", """
                # Kill wineserver for this prefix
                for ws in $(pgrep -f wineserver); do
                    kill -TERM $ws 2>/dev/null
                done
                # Also kill any wine processes
                pkill -TERM -f "winedevice" 2>/dev/null
                pkill -TERM -f "wine64-preloader" 2>/dev/null
                pkill -TERM -f "wineloader" 2>/dev/null
                # Try wineserver --kill with the bottle's prefix
                WINEPREFIX="\(bottlePath)" /Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wineserver --kill 2>/dev/null
                WINEPREFIX="\(bottlePath)" /opt/homebrew/bin/wineserver --kill 2>/dev/null
                true
                """]
            try? ps.run()
            ps.waitUntilExit()

            await MainActor.run {
                bottleRunning = false
                // Re-check after a moment
                Task {
                    try? await Task.sleep(for: .seconds(1))
                    checkWineRunning()
                }
            }
        }
    }

    /// Check if Wine is running by looking for wineserver/winedevice processes.
    private func checkWineRunning() {
        // Check via Cauldron's tracker first
        if CauldronBridge.shared.isBottleRunning(bottleId: bottle.id) {
            bottleRunning = true
            return
        }

        // Also check system-wide for any wineserver (catches CrossOver-launched processes)
        let ps = Process()
        ps.executableURL = URL(fileURLWithPath: "/usr/bin/pgrep")
        ps.arguments = ["-f", "wineserver"]
        let pipe = Pipe()
        ps.standardOutput = pipe
        ps.standardError = FileHandle.nullDevice
        do {
            try ps.run()
            ps.waitUntilExit()
            bottleRunning = ps.terminationStatus == 0
        } catch {
            bottleRunning = false
        }
    }

    /// Poll running state every 3 seconds so the UI stays current.
    private func startRunningCheck() {
        runningCheckTimer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: true) { _ in
            Task { @MainActor in
                checkWineRunning()
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

    @Environment(LicenseManager.self) private var licenseManager: LicenseManager?

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
            if !game.compatStatus.isEmpty && game.compatStatus.lowercased() != "unknown" {
                Text(game.compatStatus.capitalized)
                    .font(.caption)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 2)
                    .background(statusColor(for: game.compatStatus).opacity(0.2))
                    .foregroundStyle(statusColor(for: game.compatStatus))
                    .clipShape(Capsule())
            }
            // Play button — extracts exe path from game.notes (pipe-delimited)
            if let exePath = extractExePath(from: game) {
                Button {
                    launchGame(exePath: exePath)
                } label: {
                    Image(systemName: "play.fill")
                        .padding(6)
                }
                .buttonStyle(.plain)
                .glassEffect(.regular.tint(.green).interactive(), in: .circle)
                .disabled(!(licenseManager?.status.canLaunchGames ?? true))
                .help(licenseManager?.status.canLaunchGames ?? true ? "Launch \(game.title)" : "Activate Cauldron to launch games")
            }
        }
        .padding(.vertical, 4)
    }

    private func extractExePath(from game: GameRecord) -> String? {
        // notes field stores exe path (from game scanner)
        let notes = game.notes
        if notes.isEmpty { return nil }
        // If it contains pipe delimiter, exe path is the first component
        let components = notes.split(separator: "|").map(String.init)
        if let path = components.first, path.contains("/") || path.contains("\\") {
            return path
        }
        return notes.contains(".exe") ? notes : nil
    }

    private func launchGame(exePath: String) {
        // Build settings from global profile with the bottle's selected backend
        let settings = CauldronBridge.LaunchSettings.from(
            appSettings: .shared,
            perGame: nil,
            backend: graphicsBackend
        )
        let success = CauldronBridge.shared.launchExe(
            bottleId: bottle.id,
            exePath: exePath,
            settings: settings
        )
        if !success {
            launchErrorMessage = "Failed to launch game. Check that Wine is installed."
            showLaunchError = true
        }
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

            let settings = CauldronBridge.LaunchSettings.from(
                appSettings: .shared,
                perGame: nil,
                backend: graphicsBackend
            )
            let success = CauldronBridge.shared.launchExe(
                bottleId: bottle.id,
                exePath: url.path,
                settings: settings
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
