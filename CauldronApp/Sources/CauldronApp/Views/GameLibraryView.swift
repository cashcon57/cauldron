import SwiftUI

/// A game paired with its source bottle, enabling launch from the library.
struct LibraryGame: Identifiable, Hashable {
    var id: String { "\(bottleId)-\(game.id)" }
    let game: GameRecord
    let bottleId: String
    let bottleName: String

    static func == (lhs: LibraryGame, rhs: LibraryGame) -> Bool { lhs.id == rhs.id }
    func hash(into hasher: inout Hasher) { hasher.combine(id) }
}

struct GameLibraryView: View {
    @Environment(BottleListViewModel.self) private var viewModel
    @Environment(LicenseManager.self) private var licenseManager: LicenseManager?
    @State private var games: [LibraryGame] = []
    @State private var searchText: String = ""
    @State private var isLoading: Bool = false
    @State private var isGridView: Bool = true
    @State private var selectedGame: LibraryGame? = nil
    @State private var launchError: String? = nil
    @State private var showLaunchError = false
    @State private var launchingGameId: String? = nil

    private var filteredGames: [LibraryGame] {
        if searchText.isEmpty { return games }
        return games.filter { $0.game.title.localizedCaseInsensitiveContains(searchText) }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Search bar + controls
            HStack(spacing: 12) {
                HStack {
                    Image(systemName: "magnifyingglass")
                        .foregroundStyle(.secondary)
                    TextField("Search games...", text: $searchText)
                        .textFieldStyle(.plain)
                    if !searchText.isEmpty {
                        Button {
                            searchText = ""
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundStyle(.secondary)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .glassEffect(.regular, in: .capsule)

                Spacer()

                Button {
                    loadGames()
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.plain)
                .help("Rescan all bottles")

                Text("\(games.count) game\(games.count == 1 ? "" : "s")")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                HStack(spacing: 2) {
                    Button {
                        withAnimation { isGridView = true }
                    } label: {
                        Image(systemName: "square.grid.2x2")
                    }
                    .buttonStyle(.plain)
                    .glassEffect(
                        isGridView ? .regular.tint(.accentColor).interactive() : .regular.interactive(),
                        in: .rect(cornerRadius: 6)
                    )

                    Button {
                        withAnimation { isGridView = false }
                    } label: {
                        Image(systemName: "list.bullet")
                    }
                    .buttonStyle(.plain)
                    .glassEffect(
                        !isGridView ? .regular.tint(.accentColor).interactive() : .regular.interactive(),
                        in: .rect(cornerRadius: 6)
                    )
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)

            if isLoading {
                Spacer()
                ProgressView("Scanning bottles for games...")
                Spacer()
            } else if games.isEmpty {
                Spacer()
                ContentUnavailableView(
                    "No Games Detected",
                    systemImage: "gamecontroller",
                    description: Text("Import a bottle with Steam installed, then games will appear here automatically.")
                )
                Spacer()
            } else if filteredGames.isEmpty {
                Spacer()
                ContentUnavailableView.search(text: searchText)
                Spacer()
            } else if isGridView {
                gridContent
            } else {
                listContent
            }
        }
        .navigationTitle("Game Library")
        .onAppear { loadGames() }
        .sheet(item: $selectedGame) { libGame in
            GameSettingsSheet(libraryGame: libGame, onLaunch: { launchGame(libGame) })
        }
        .alert("Launch Error", isPresented: $showLaunchError) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(launchError ?? "Unknown error")
        }
    }

    // MARK: - Grid

    private var gridContent: some View {
        ScrollView {
            LazyVGrid(
                columns: [GridItem(.adaptive(minimum: 200, maximum: 280), spacing: 16)],
                spacing: 16
            ) {
                ForEach(filteredGames) { libGame in
                    let canPlay = licenseManager?.status.canLaunchGames ?? true
                    GameCardView(
                        libraryGame: libGame,
                        canLaunch: canPlay,
                        isLaunching: launchingGameId == libGame.id,
                        onPlay: { launchGame(libGame) },
                        onSettings: { selectedGame = libGame }
                    )
                }
            }
            .padding()
        }
    }

    // MARK: - List

    private var listContent: some View {
        ScrollView {
            LazyVStack(spacing: 2) {
                ForEach(filteredGames) { libGame in
                    GameRowView(
                        libraryGame: libGame,
                        canLaunch: licenseManager?.status.canLaunchGames ?? true,
                        isLaunching: launchingGameId == libGame.id,
                        onPlay: { launchGame(libGame) },
                        onSettings: { selectedGame = libGame }
                    )
                }
            }
            .padding(.horizontal)
        }
    }

    // MARK: - Launch

    private func launchGame(_ libGame: LibraryGame) {
        guard let exePath = extractExePath(from: libGame.game) else {
            launchError = "Could not determine executable path for \(libGame.game.title)."
            showLaunchError = true
            return
        }

        // Prevent duplicate launches
        if launchingGameId == libGame.id { return }

        if CauldronBridge.shared.isBottleRunning(bottleId: libGame.bottleId) {
            let isSteam = exePath.lowercased().contains("steam")
            if !isSteam {
                launchError = "\(libGame.game.title) is already running in this bottle. Stop it first or wait for it to exit."
                showLaunchError = true
                return
            }
        }

        launchingGameId = libGame.id

        let gameKey = gameKeyFor(libGame)
        let perGame = PerGameSettings(gameKey: gameKey)
        let settings = CauldronBridge.LaunchSettings.from(
            appSettings: .shared,
            perGame: perGame.hasOverrides ? perGame : nil
        )
        let bottleId = libGame.bottleId
        let title = libGame.game.title

        Task {
            let success = CauldronBridge.shared.launchExe(
                bottleId: bottleId,
                exePath: exePath,
                settings: settings
            )

            try? await Task.sleep(for: .seconds(3))
            launchingGameId = nil
            if !success {
                launchError = "Failed to launch \(title). Check that Wine is installed."
                showLaunchError = true
            }
        }
    }

    private func gameKeyFor(_ libGame: LibraryGame) -> String {
        if let appId = libGame.game.steamAppId, appId > 0 {
            return "steam_\(appId)"
        }
        return "title_\(libGame.game.title)"
    }

    // MARK: - Data Loading

    private func loadGames() {
        isLoading = true
        Task {
            var allGames: [LibraryGame] = []
            for bottle in viewModel.bottles {
                let bottleGames = CauldronBridge.shared.scanBottleGames(bottleId: bottle.id)
                let filtered = bottleGames.filter { $0.knownIssues == "game" }
                for game in filtered {
                    allGames.append(LibraryGame(
                        game: game,
                        bottleId: bottle.id,
                        bottleName: bottle.name
                    ))
                }
            }
            games = allGames
            isLoading = false
        }
    }
}

// MARK: - Game Card (Grid)

private struct GameCardView: View {
    let libraryGame: LibraryGame
    let canLaunch: Bool
    let isLaunching: Bool
    let onPlay: () -> Void
    let onSettings: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Steam header image — tap opens settings
            SteamHeaderImage(appId: libraryGame.game.steamAppId)
                .frame(height: 94)
                .clipped()
                .contentShape(Rectangle())
                .onTapGesture { onSettings() }

            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(libraryGame.game.title)
                        .font(.subheadline.weight(.semibold))
                        .lineLimit(1)
                        .foregroundStyle(.primary)

                    HStack(spacing: 4) {
                        let apis = extractAllAPIs(from: libraryGame.game.notes)
                        if !apis.isEmpty {
                            ForEach(apis, id: \.self) { api in
                                Text(api)
                                    .font(.system(size: 9, weight: .medium))
                                    .padding(.horizontal, 4)
                                    .padding(.vertical, 1)
                                    .background(apiColor(api).opacity(0.2))
                                    .foregroundStyle(apiColor(api))
                                    .clipShape(Capsule())
                            }
                        }
                        Text(libraryGame.bottleName)
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                            .lineLimit(1)
                    }
                }
                .contentShape(Rectangle())
                .onTapGesture { onSettings() }

                Spacer()

                // Play button
                ZStack {
                    if isLaunching {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Circle()
                            .fill(canLaunch ? Color.green.gradient : Color.gray.gradient)
                            .frame(width: 32, height: 32)
                            .overlay {
                                Image(systemName: "play.fill")
                                    .font(.system(size: 14))
                                    .foregroundStyle(.white)
                                    .offset(x: 1)
                            }
                            .shadow(radius: 3)
                            .onTapGesture { if canLaunch { onPlay() } }
                    }
                }
                .frame(width: 32, height: 32)
            }
            .padding(10)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.ultraThinMaterial, in: .rect(cornerRadius: 12))
    }
}

// MARK: - Game Row (List)

private struct GameRowView: View {
    let libraryGame: LibraryGame
    let canLaunch: Bool
    let isLaunching: Bool
    let onPlay: () -> Void
    let onSettings: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            SteamHeaderImage(appId: libraryGame.game.steamAppId)
                .frame(width: 64, height: 30)
                .clipShape(RoundedRectangle(cornerRadius: 4))

            VStack(alignment: .leading, spacing: 2) {
                Text(libraryGame.game.title)
                    .font(.body.weight(.medium))
                HStack(spacing: 4) {
                    let apis = extractAllAPIs(from: libraryGame.game.notes)
                    if !apis.isEmpty {
                        ForEach(apis, id: \.self) { api in
                            Text(api)
                                .font(.system(size: 9, weight: .medium))
                                .padding(.horizontal, 4)
                                .padding(.vertical, 1)
                                .background(apiColor(api).opacity(0.2))
                                .foregroundStyle(apiColor(api))
                                .clipShape(Capsule())
                        }
                    } else if let gfx = GraphicsBackend(rawValue: libraryGame.game.backend), gfx != .auto {
                        Text(gfx.displayName)
                            .font(.system(size: 9, weight: .medium))
                            .padding(.horizontal, 4)
                            .padding(.vertical, 1)
                            .background(gfx.tintColor.opacity(0.2))
                            .foregroundStyle(gfx.tintColor)
                            .clipShape(Capsule())
                    }
                    Text(libraryGame.bottleName)
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }
            }

            Spacer()

            if isLaunching {
                ProgressView()
                    .controlSize(.small)
                    .padding(6)
            } else {
                Image(systemName: "play.fill")
                    .padding(6)
                    .background(canLaunch ? Color.green.opacity(0.2) : Color.gray.opacity(0.2))
                    .clipShape(Circle())
                    .onTapGesture { if canLaunch { onPlay() } }
            }

            Image(systemName: "gearshape")
                .padding(6)
                .background(Color.secondary.opacity(0.2))
                .clipShape(Circle())
                .onTapGesture { onSettings() }
        }
        .padding(.vertical, 6)
        .padding(.horizontal, 8)
        .background(.ultraThinMaterial, in: .rect(cornerRadius: 8))
    }
}

// MARK: - Steam Header Image

private struct SteamHeaderImage: View {
    let appId: Int?

    var body: some View {
        if let appId = appId, appId > 0 {
            AsyncImage(url: URL(string: "https://cdn.cloudflare.steamstatic.com/steam/apps/\(appId)/header.jpg")) { phase in
                switch phase {
                case .success(let image):
                    image
                        .resizable()
                        .aspectRatio(contentMode: .fill)
                case .failure:
                    placeholderView
                case .empty:
                    ProgressView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                @unknown default:
                    placeholderView
                }
            }
        } else {
            placeholderView
        }
    }

    private var placeholderView: some View {
        ZStack {
            Color.gray.opacity(0.15)
            Image(systemName: "gamecontroller.fill")
                .font(.title2)
                .foregroundStyle(.quaternary)
        }
    }
}

// MARK: - Game Settings Sheet

private struct GameSettingsSheet: View {
    let libraryGame: LibraryGame
    let onLaunch: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var gameSettings: PerGameSettings? = nil
    @State private var patchStatus: [CauldronBridge.GamePatchResult] = []
    @State private var backendSwitchError: String? = nil
    @State private var isPatching = false
    @State private var showAdvanced = false

    private var settings: AppSettings { .shared }

    private var gameKey: String {
        if let appId = libraryGame.game.steamAppId, appId > 0 {
            return "steam_\(appId)"
        }
        return "title_\(libraryGame.game.title)"
    }

    private var profileMismatches: [SettingDifference] {
        gameSettings?.mismatchesFromProfile(settings.activeProfile) ?? []
    }

    @State private var gameRecommendation: CauldronBridge.GameRecommendation? = nil

    private var recommendedBackend: String? {
        // 1. Check DB recommendation first (community-curated)
        if let rec = gameRecommendation, rec.found, let backend = rec.backend {
            return backend
        }
        // 2. Fall back to DX-based heuristic
        let dx = extractDX(from: libraryGame.game.notes)
        switch dx {
        case "DX12": return "D3DMetal"
        case "DX11": return "DXMT"
        case "DX10": return "DXVK + MoltenVK"
        case "DX9": return "DXVK + MoltenVK"
        case "DX8": return "DXVK + MoltenVK"
        case "Vulkan": return "Native Vulkan"
        case "OpenGL": return "WineD3D"
        default: return nil
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            ZStack(alignment: .topTrailing) {
                SteamHeaderImage(appId: libraryGame.game.steamAppId)
                    .frame(height: 130)
                    .clipped()

                Button { dismiss() } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.title3)
                        .symbolRenderingMode(.palette)
                        .foregroundStyle(.white, .black.opacity(0.5))
                }
                .buttonStyle(.plain)
                .padding(10)
            }

            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    // Title + Play
                    HStack {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(libraryGame.game.title)
                                .font(.title3.weight(.bold))
                            HStack(spacing: 8) {
                                if let dx = extractDX(from: libraryGame.game.notes) {
                                    Text(dx)
                                        .font(.caption)
                                        .padding(.horizontal, 6)
                                        .padding(.vertical, 2)
                                        .background(dxColor(dx).opacity(0.2))
                                        .foregroundStyle(dxColor(dx))
                                        .clipShape(Capsule())
                                }
                                Text(libraryGame.bottleName)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        Spacer()
                        Button {
                            onLaunch()
                            dismiss()
                        } label: {
                            Label("Play", systemImage: "play.fill")
                                .font(.headline)
                                .padding(.horizontal, 20)
                                .padding(.vertical, 10)
                        }
                        .buttonStyle(.plain)
                        .glassEffect(.regular.tint(.green).interactive(), in: .capsule)
                    }

                    // Profile mismatch warning
                    if !profileMismatches.isEmpty {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack(spacing: 6) {
                                Image(systemName: "exclamationmark.triangle.fill")
                                    .foregroundStyle(.orange)
                                Text("Settings differ from \(settings.activeProfile.displayName) profile")
                                    .font(.subheadline.weight(.medium))
                                Spacer()
                                Button("Reset") {
                                    gameSettings?.resetToGlobal()
                                }
                                .font(.caption)
                                .buttonStyle(.plain)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 3)
                                .glassEffect(.regular.tint(.orange).interactive(), in: .capsule)
                            }

                            ForEach(profileMismatches) { diff in
                                HStack(spacing: 4) {
                                    Text(diff.name)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                    Spacer()
                                    Text(diff.current)
                                        .font(.caption.weight(.medium))
                                        .foregroundStyle(.orange)
                                    Text("(profile: \(diff.expected))")
                                        .font(.caption2)
                                        .foregroundStyle(.tertiary)
                                }
                            }
                        }
                        .padding(10)
                        .glassEffect(.regular.tint(.orange), in: .rect(cornerRadius: 10))
                    }

                    Divider()

                    // Quick settings (always visible)
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("Graphics Backend")
                                .font(.headline)
                            Spacer()
                            if let rec = recommendedBackend {
                                Text("Rec: \(rec)")
                                    .font(.caption)
                                    .foregroundStyle(.green)
                            }
                        }

                        if let gs = gameSettings {
                            let currentBackend = gs.graphicsBackend ?? settings.defaultGraphicsBackend
                            Picker("Backend", selection: Binding(
                                get: { currentBackend },
                                set: { newBackend in
                                    gs.graphicsBackend = newBackend
                                    gs.save()
                                    // Install the backend's DLLs into the bottle
                                    let bridge = CauldronBridge.shared
                                    let bid = libraryGame.bottleId
                                    Task.detached {
                                        let result = bridge.switchBackend(bottleId: bid, backend: newBackend)
                                        if let result, !result.success {
                                            await MainActor.run {
                                                backendSwitchError = result.error
                                            }
                                        }
                                    }
                                }
                            )) {
                                ForEach(GraphicsBackend.allCases, id: \.self) { backend in
                                    Text(backend.displayName).tag(backend)
                                }
                            }

                            if let err = backendSwitchError {
                                Text(err)
                                    .font(.caption)
                                    .foregroundStyle(.red)
                            }

                            if gs.graphicsBackend != nil {
                                HStack(spacing: 4) {
                                    Image(systemName: "arrow.uturn.backward.circle")
                                        .font(.caption2)
                                    Text("Custom override active")
                                        .font(.caption2)
                                    Button("Clear") {
                                        gs.graphicsBackend = nil; gs.save()
                                    }
                                    .font(.caption2)
                                    .foregroundStyle(.blue)
                                }
                                .foregroundStyle(.secondary)
                            }
                        }
                    }

                    Divider()

                    // Advanced per-game overrides (collapsible)
                    DisclosureGroup("Per-Game Overrides", isExpanded: $showAdvanced) {
                        if let gs = gameSettings {
                            VStack(alignment: .leading, spacing: 10) {
                                Text("These override global profile settings for this game only. Leave toggles in their default state to inherit from the \(settings.activeProfile.displayName) profile.")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)

                                overrideToggle("RosettaX87", binding: Binding(
                                    get: { gs.rosettaX87Enabled ?? settings.rosettaX87Enabled },
                                    set: { gs.rosettaX87Enabled = $0; gs.save() }
                                ), isOverridden: gs.rosettaX87Enabled != nil) {
                                    gs.rosettaX87Enabled = nil; gs.save()
                                }

                                overrideToggle("Async Shader Compilation", binding: Binding(
                                    get: { gs.asyncShaderCompilation ?? settings.asyncShaderCompilation },
                                    set: { gs.asyncShaderCompilation = $0; gs.save() }
                                ), isOverridden: gs.asyncShaderCompilation != nil) {
                                    gs.asyncShaderCompilation = nil; gs.save()
                                }

                                overrideToggle("MetalFX Upscaling", binding: Binding(
                                    get: { gs.metalFXSpatialUpscaling ?? settings.metalFXSpatialUpscaling },
                                    set: { gs.metalFXSpatialUpscaling = $0; gs.save() }
                                ), isOverridden: gs.metalFXSpatialUpscaling != nil) {
                                    gs.metalFXSpatialUpscaling = nil; gs.save()
                                }

                                overrideToggle("DXR Ray Tracing", binding: Binding(
                                    get: { gs.dxrRayTracing ?? settings.dxrRayTracing },
                                    set: { gs.dxrRayTracing = $0; gs.save() }
                                ), isOverridden: gs.dxrRayTracing != nil) {
                                    gs.dxrRayTracing = nil; gs.save()
                                }

                                overrideToggle("MoltenVK Arg Buffers", binding: Binding(
                                    get: { gs.moltenVKArgumentBuffers ?? settings.moltenVKArgumentBuffers },
                                    set: { gs.moltenVKArgumentBuffers = $0; gs.save() }
                                ), isOverridden: gs.moltenVKArgumentBuffers != nil) {
                                    gs.moltenVKArgumentBuffers = nil; gs.save()
                                }

                                overrideToggle("FSR Upscaling", binding: Binding(
                                    get: { gs.fsrEnabled ?? false },
                                    set: { gs.fsrEnabled = $0; gs.save() }
                                ), isOverridden: gs.fsrEnabled != nil) {
                                    gs.fsrEnabled = nil; gs.save()
                                }

                                overrideToggle("Large Address Aware", binding: Binding(
                                    get: { gs.largeAddressAware ?? false },
                                    set: { gs.largeAddressAware = $0; gs.save() }
                                ), isOverridden: gs.largeAddressAware != nil) {
                                    gs.largeAddressAware = nil; gs.save()
                                }

                                overrideToggle("Auto-Apply Game Patches", binding: Binding(
                                    get: { gs.autoApplyGamePatches ?? settings.autoApplyGamePatches },
                                    set: { gs.autoApplyGamePatches = $0; gs.save() }
                                ), isOverridden: gs.autoApplyGamePatches != nil) {
                                    gs.autoApplyGamePatches = nil; gs.save()
                                }

                                if gs.hasOverrides {
                                    Button("Reset All to Profile Defaults") {
                                        gs.resetToGlobal()
                                    }
                                    .font(.caption)
                                    .foregroundStyle(.red)
                                    .padding(.top, 4)
                                }
                            }
                            .padding(.top, 6)
                        }
                    }
                    .font(.headline)

                    // Game binary patches — filtered to this game only
                    if !relevantPatches.isEmpty {
                        Divider()
                        VStack(alignment: .leading, spacing: 8) {
                            Text("macOS Compatibility Fixes")
                                .font(.headline)

                            ForEach(Array(relevantPatches.enumerated()), id: \.offset) { _, patch in
                                HStack {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(patch.gameTitle ?? "Unknown")
                                            .font(.subheadline.weight(.medium))
                                        Text("\(patch.patchesAvailable ?? 0) fix\(patch.patchesAvailable == 1 ? "" : "es") available")
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                    }
                                    Spacer()

                                    if patch.alreadyPatched == true {
                                        HStack(spacing: 4) {
                                            Image(systemName: "checkmark.circle.fill")
                                                .foregroundStyle(.green)
                                            Text("Applied")
                                                .font(.caption)
                                                .foregroundStyle(.green)
                                        }
                                        if patch.canRestore == true, let exePath = patch.exePath {
                                            Button("Restore") {
                                                let bridge = CauldronBridge.shared
                                                Task.detached {
                                                    let _ = bridge.restoreGameExe(exePath: exePath)
                                                    await MainActor.run { checkPatches() }
                                                }
                                            }
                                            .font(.caption)
                                            .foregroundStyle(.orange)
                                        }
                                    } else if let exePath = patch.exePath {
                                        Button {
                                            isPatching = true
                                            let bridge = CauldronBridge.shared
                                            Task.detached {
                                                let _ = bridge.applyGamePatch(exePath: exePath)
                                                await MainActor.run {
                                                    isPatching = false
                                                    checkPatches()
                                                }
                                            }
                                        } label: {
                                            if isPatching {
                                                ProgressView().controlSize(.small)
                                            } else {
                                                Text("Apply Fix")
                                                    .padding(.horizontal, 10)
                                                    .padding(.vertical, 4)
                                            }
                                        }
                                        .buttonStyle(.plain)
                                        .glassEffect(.regular.tint(.blue).interactive(), in: .capsule)
                                        .disabled(isPatching)
                                    }
                                }
                            }
                        }
                    }

                    Divider()

                    // Info
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Info")
                            .font(.headline)
                        if let appId = libraryGame.game.steamAppId {
                            HStack {
                                Text("Steam App ID").foregroundStyle(.secondary)
                                Spacer()
                                Text("\(appId)").font(.body.monospaced())
                            }
                        }
                        HStack {
                            Text("Bottle").foregroundStyle(.secondary)
                            Spacer()
                            Text(libraryGame.bottleName)
                        }
                        HStack {
                            Text("Profile").foregroundStyle(.secondary)
                            Spacer()
                            HStack(spacing: 4) {
                                Image(systemName: settings.activeProfile.icon)
                                    .foregroundStyle(settings.activeProfile.tintColor)
                                Text(settings.activeProfile.displayName)
                            }
                        }
                    }
                }
                .padding(20)
            }
        }
        .frame(width: 500, height: 740)
        .onAppear {
            gameSettings = PerGameSettings(gameKey: gameKey)
            checkPatches()
            if let appId = libraryGame.game.steamAppId, appId > 0 {
                gameRecommendation = CauldronBridge.shared.getGameRecommendation(appId: UInt32(appId))
            }
        }
    }

    /// Only show patches relevant to this specific game, not all games in the bottle.
    private var relevantPatches: [CauldronBridge.GamePatchResult] {
        let title = libraryGame.game.title.lowercased()
        return patchStatus.filter { patch in
            guard let patchTitle = patch.gameTitle?.lowercased() else { return false }
            return patchTitle.contains(title) || title.contains(patchTitle)
        }
    }

    private func checkPatches() {
        patchStatus = CauldronBridge.shared.scanGamePatches(bottleId: libraryGame.bottleId)
    }

    @ViewBuilder
    private func overrideToggle(_ label: String, binding: Binding<Bool>, isOverridden: Bool, onClear: @escaping () -> Void) -> some View {
        HStack {
            Toggle(label, isOn: binding)
                .font(.subheadline)
            if isOverridden {
                Button {
                    onClear()
                } label: {
                    Image(systemName: "arrow.uturn.backward.circle.fill")
                        .foregroundStyle(.blue)
                        .font(.caption)
                }
                .buttonStyle(.plain)
                .help("Clear override, inherit from profile")
            }
        }
    }
}

// MARK: - Helpers

private func extractExePath(from game: GameRecord) -> String? {
    let notes = game.notes
    if notes.isEmpty { return nil }
    let components = notes.split(separator: "|").map { $0.trimmingCharacters(in: .whitespaces) }
    // Format: "DX11 | /path/to/exe" or just "/path/to/exe"
    if components.count >= 2, components[1].contains("/") || components[1].contains("\\") {
        return components[1]
    }
    if let first = components.first, first.contains("/") || first.contains("\\") {
        return first
    }
    return notes.contains(".exe") ? notes : nil
}

/// Extract the graphics API label from the notes field.
/// Format: "DX11, Vulkan | /path/to/exe" or "DX9 | /path" or just "/path"
private func extractGraphicsLabel(from notes: String) -> String? {
    // Must start with a known API prefix
    let prefixes = ["DX", "Vulkan", "OpenGL"]
    guard prefixes.contains(where: { notes.hasPrefix($0) }) else { return nil }
    return String(notes.prefix(while: { $0 != "|" })).trimmingCharacters(in: .whitespaces)
}

/// Extract just the primary DX label for simple display.
private func extractDX(from notes: String) -> String? {
    guard let label = extractGraphicsLabel(from: notes) else { return nil }
    // Return the first API tag
    let first = label.split(separator: ",").first?.trimmingCharacters(in: .whitespaces)
    return first
}

/// Split the full graphics label into individual API tags for badge display.
private func extractAllAPIs(from notes: String) -> [String] {
    guard let label = extractGraphicsLabel(from: notes) else { return [] }
    return label.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }
}

private func apiColor(_ api: String) -> Color {
    switch api {
    case "DX12": return .purple
    case "DX11": return .blue
    case "DX10": return .cyan
    case "DX9": return .orange
    case "DX8": return .brown
    case "Vulkan": return .red
    case "OpenGL": return .green
    default: return .secondary
    }
}

private func dxColor(_ dx: String) -> Color { apiColor(dx) }

