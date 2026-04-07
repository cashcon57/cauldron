import SwiftUI

enum SidebarItem: Hashable {
    case bottle(Bottle)
    case gameLibrary
    case syncStatus
}

struct ContentView: View {
    @Environment(BottleListViewModel.self) private var viewModel
    @State private var selectedItem: SidebarItem? = nil
    @State private var showingSteamInstaller = false
    @State private var showingDiscoveredBottles = false

    var body: some View {
        @Bindable var vm = viewModel

        NavigationSplitView {
            List(selection: $selectedItem) {
                Section {
                    ForEach(viewModel.bottles) { bottle in
                        NavigationLink(value: SidebarItem.bottle(bottle)) {
                            BottleRow(bottle: bottle)
                        }
                        .contextMenu {
                            Button(role: .destructive) {
                                viewModel.deleteBottle(bottle)
                            } label: {
                                Label("Delete", systemImage: "trash")
                            }
                        }
                    }

                    if viewModel.bottles.isEmpty {
                        Text("No bottles yet")
                            .foregroundStyle(.tertiary)
                            .font(.subheadline)
                    }
                } header: {
                    Text("Bottles")
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .padding(.top, 4)
                }

                Section {
                    NavigationLink(value: SidebarItem.gameLibrary) {
                        Label("Game Library", systemImage: "gamecontroller")
                    }
                } header: {
                    Text("Library")
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .padding(.top, 8)
                }

                Section {
                    NavigationLink(value: SidebarItem.syncStatus) {
                        Label("Patches", systemImage: "arrow.triangle.2.circlepath")
                    }
                } header: {
                    Text("Updates")
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .padding(.top, 8)
                }
            }
            .navigationSplitViewColumnWidth(min: 220, ideal: 260, max: 340)
            .navigationTitle("Cauldron")
        } detail: {
            switch selectedItem {
            case .bottle(let bottle):
                BottleDetailView(bottle: bottle)
            case .gameLibrary:
                GameLibraryView()
            case .syncStatus:
                SyncStatusView()
            case .none:
                ContentUnavailableView(
                    "Welcome to Cauldron",
                    systemImage: "sparkles",
                    description: Text("Select a bottle or section from the sidebar.")
                )
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Button {
                    viewModel.isCreatingBottle = true
                } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 12, weight: .medium))
                        .frame(width: 24, height: 24)
                }
                .help("Create a new Wine bottle")
                .keyboardShortcut("n")

                Button {
                    showingSteamInstaller = true
                } label: {
                    Image(systemName: "gamecontroller.fill")
                        .font(.system(size: 12, weight: .medium))
                        .frame(width: 24, height: 24)
                }
                .help("Install Steam into a bottle")

                Button {
                    showingDiscoveredBottles = true
                } label: {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 12, weight: .medium))
                        .frame(width: 24, height: 24)
                }
                .help("Scan for existing Wine bottles from other apps")
            }
        }
        .sheet(isPresented: $vm.isCreatingBottle) {
            CreateBottleView()
        }
        .sheet(isPresented: $showingSteamInstaller) {
            SteamInstallWizard()
        }
        .sheet(isPresented: $showingDiscoveredBottles) {
            DiscoveredBottlesView()
        }
        .onAppear {
            // Auto-select the first bottle if available
            if let first = viewModel.bottles.first {
                selectedItem = .bottle(first)
            }
        }
        .onChange(of: viewModel.bottles) { _, newBottles in
            // If current selection is a deleted bottle, clear it
            if case .bottle(let b) = selectedItem, !newBottles.contains(where: { $0.id == b.id }) {
                selectedItem = newBottles.first.map { .bottle($0) }
            }
            // Auto-select first bottle if nothing selected
            if selectedItem == nil, let first = newBottles.first {
                selectedItem = .bottle(first)
            }
        }
    }
}

private struct BottleRow: View {
    let bottle: Bottle

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(bottle.name)
                .font(.headline)

            HStack(spacing: 6) {
                Text(bottle.wineVersion)
                    .font(.caption)
                    .foregroundStyle(.secondary)

                let backend = GraphicsBackend(rawValue: bottle.graphicsBackend)
                Text(backend?.displayName ?? bottle.graphicsBackend)
                    .font(.caption2)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .glassEffect(.regular.tint(backend?.tintColor ?? .secondary), in: .capsule)
            }
        }
        .padding(.vertical, 2)
    }
}
