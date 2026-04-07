import SwiftUI

struct GameLibraryView: View {
    @Environment(BottleListViewModel.self) private var viewModel
    @State private var games: [GameRecord] = []
    @State private var searchText: String = ""
    @State private var isLoading: Bool = false
    @State private var isGridView: Bool = true
    @State private var selectedGame: GameRecord? = nil

    private var filteredGames: [GameRecord] {
        if searchText.isEmpty { return games }
        return games.filter { $0.title.localizedCaseInsensitiveContains(searchText) }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Search bar
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
        .sheet(item: $selectedGame) { game in
            GameSettingsSheet(game: game)
        }
    }

    // MARK: - Grid

    private var gridContent: some View {
        ScrollView {
            LazyVGrid(
                columns: [GridItem(.adaptive(minimum: 200, maximum: 280), spacing: 16)],
                spacing: 16
            ) {
                ForEach(filteredGames) { game in
                    GameCardView(game: game) {
                        selectedGame = game
                    }
                }
            }
            .padding()
        }
    }

    // MARK: - List

    private var listContent: some View {
        List(filteredGames) { game in
            GameRowView(game: game)
                .contentShape(Rectangle())
                .onTapGesture { selectedGame = game }
        }
    }

    // MARK: - Data Loading

    private func loadGames() {
        isLoading = true
        Task {
            var allGames: [GameRecord] = []
            for bottle in viewModel.bottles {
                let bottleGames = CauldronBridge.shared.scanBottleGames(bottleId: bottle.id)
                let filtered = bottleGames.filter { $0.knownIssues == "game" }
                allGames.append(contentsOf: filtered)
            }
            games = allGames
            isLoading = false
        }
    }
}

// MARK: - Game Card (Grid)

private struct GameCardView: View {
    let game: GameRecord
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            VStack(alignment: .leading, spacing: 0) {
                // Steam header image
                SteamHeaderImage(appId: game.steamAppId)
                    .frame(height: 94)
                    .clipped()

                VStack(alignment: .leading, spacing: 4) {
                    Text(game.title)
                        .font(.subheadline.weight(.semibold))
                        .lineLimit(1)
                        .foregroundStyle(.primary)

                    Text(extractDX(from: game.notes) ?? " ")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                .padding(10)
                .frame(height: 48, alignment: .top)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
            .glassEffect(.regular, in: .rect(cornerRadius: 12))
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Game Row (List)

private struct GameRowView: View {
    let game: GameRecord

    var body: some View {
        HStack(spacing: 12) {
            // Small icon
            SteamHeaderImage(appId: game.steamAppId)
                .frame(width: 64, height: 30)
                .clipShape(RoundedRectangle(cornerRadius: 4))

            VStack(alignment: .leading, spacing: 2) {
                Text(game.title)
                    .font(.body.weight(.medium))
                if let dx = extractDX(from: game.notes) {
                    Text(dx)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            Spacer()
        }
        .padding(.vertical, 2)
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
    let game: GameRecord
    @Environment(\.dismiss) private var dismiss
    @State private var selectedBackend: String = "Auto"

    private struct BackendOption {
        let tag: String
        let name: String
        let description: String
    }

    private var backendOptions: [BackendOption] {
        [
            BackendOption(tag: "Auto", name: "Auto", description: "Let Cauldron choose the best backend based on the game's DirectX version and your hardware."),
            BackendOption(tag: "D3DMetal", name: "D3DMetal (Apple GPTK)", description: "Apple's Game Porting Toolkit translation layer. Best for DX11/DX12 games on Apple Silicon. Native Metal performance."),
            BackendOption(tag: "DXMT", name: "DXMT", description: "Community DX11-to-Metal translator. Often faster than D3DMetal for DX11 games. Active development by CrossOver community."),
            BackendOption(tag: "DxvkMoltenVK", name: "DXVK + MoltenVK", description: "DX9/10/11 → Vulkan → Metal chain. Most compatible option for older DX9/DX10 games. Slight overhead from double translation."),
            BackendOption(tag: "DxvkKosmicKrisp", name: "DXVK + Kosmic Krisp", description: "DX9/10/11 via Mesa's native Vulkan 1.3 on Metal 4. Best performance for DX9-11 games on macOS 26+ with M-series chips."),
            BackendOption(tag: "Vkd3dProton", name: "VKD3D-Proton", description: "DX12-to-Vulkan translation. Required for DX12-only games. Works via MoltenVK on macOS. Performance varies by title."),
        ]
    }

    private var recommendedBackend: String? {
        let dx = extractDX(from: game.notes)
        switch dx {
        case "DX12": return "D3DMetal"
        case "DX11": return "DXMT"
        case "DX10": return "DxvkMoltenVK"
        case "DX9": return "DxvkMoltenVK"
        default: return nil
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header with game image + close button
            ZStack(alignment: .topTrailing) {
                SteamHeaderImage(appId: game.steamAppId)
                    .frame(height: 140)
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
                VStack(alignment: .leading, spacing: 16) {
                    Text(game.title)
                        .font(.title3.weight(.bold))

                    if let dx = extractDX(from: game.notes) {
                        Text(dx)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }

                    Divider()

                // Graphics backend override
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text("Graphics Backend")
                            .font(.headline)
                        Spacer()
                        if let rec = recommendedBackend {
                            Text("Recommended: \(rec)")
                                .font(.caption)
                                .foregroundStyle(.green)
                        }
                    }

                    ForEach(backendOptions, id: \.tag) { option in
                        Button {
                            selectedBackend = option.tag
                        } label: {
                            HStack(spacing: 10) {
                                Image(systemName: selectedBackend == option.tag ? "circle.inset.filled" : "circle")
                                    .foregroundStyle(selectedBackend == option.tag ? .blue : .secondary)
                                VStack(alignment: .leading, spacing: 2) {
                                    HStack(spacing: 6) {
                                        Text(option.name)
                                            .font(.subheadline.weight(.medium))
                                            .foregroundStyle(.primary)
                                        if option.tag == recommendedBackend {
                                            Text("Recommended")
                                                .font(.system(size: 9, weight: .semibold))
                                                .padding(.horizontal, 5)
                                                .padding(.vertical, 1)
                                                .background(Color.green.opacity(0.2))
                                                .foregroundStyle(.green)
                                                .clipShape(Capsule())
                                        }
                                    }
                                    Text(option.description)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                            }
                            .padding(.vertical, 4)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }

                Divider()

                // Info
                VStack(alignment: .leading, spacing: 6) {
                    Text("Info")
                        .font(.headline)

                    if let appId = game.steamAppId {
                        HStack {
                            Text("Steam App ID")
                                .foregroundStyle(.secondary)
                            Spacer()
                            Text("\(appId)")
                                .font(.body.monospaced())
                        }
                    }
                }

                }
                .padding(20)
            }
        }
        .frame(width: 480, height: 640)
        .onAppear {
            selectedBackend = game.backend
        }
    }
}

// MARK: - Helpers

private func extractDX(from notes: String) -> String? {
    // Notes format: "DX11 | /path/to/exe" or just "/path/to/exe"
    if notes.hasPrefix("DX") {
        return String(notes.prefix(while: { $0 != "|" })).trimmingCharacters(in: .whitespaces)
    }
    return nil
}
