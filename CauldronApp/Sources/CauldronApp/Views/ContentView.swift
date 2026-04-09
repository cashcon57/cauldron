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
    @State private var wineRunning = false
    @State private var wineCheckTimer: Timer? = nil

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
                if wineRunning {
                    Button {
                        killAllWine()
                    } label: {
                        HStack(spacing: 4) {
                            Circle()
                                .fill(.green)
                                .frame(width: 6, height: 6)
                            Image(systemName: "stop.fill")
                                .font(.system(size: 10, weight: .medium))
                        }
                        .frame(width: 24, height: 24)
                    }
                    .help("Kill all Wine processes")
                    .keyboardShortcut("k", modifiers: [.command, .shift])
                }

                Button {
                    viewModel.isCreatingBottle = true
                } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 12, weight: .medium))
                        .frame(width: 24, height: 24)
                }
                .help("Create a new Wine bottle")
                .keyboardShortcut("n")

                if let steamBottle = viewModel.bottles.first(where: { $0.name == "Steam" }) {
                    Button {
                        launchSteam(bottle: steamBottle)
                    } label: {
                        SteamLogoShape()
                            .fill(.primary)
                            .frame(width: 14, height: 14)
                            .frame(width: 24, height: 24)
                    }
                    .help("Launch Steam")
                    .keyboardShortcut("l", modifiers: [.command, .shift])
                } else {
                    Button {
                        showingSteamInstaller = true
                    } label: {
                        Image(systemName: "gamecontroller.fill")
                            .font(.system(size: 12, weight: .medium))
                            .frame(width: 24, height: 24)
                    }
                    .help("Install Steam into a bottle")
                }

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
            if let first = viewModel.bottles.first {
                selectedItem = .bottle(first)
            }
            checkWineRunning()
            wineCheckTimer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: true) { _ in
                Task { @MainActor in checkWineRunning() }
            }
        }
        .onDisappear { wineCheckTimer?.invalidate() }
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

    private func checkWineRunning() {
        let ps = Process()
        ps.executableURL = URL(fileURLWithPath: "/usr/bin/pgrep")
        ps.arguments = ["-f", "wineserver"]
        ps.standardOutput = FileHandle.nullDevice
        ps.standardError = FileHandle.nullDevice
        do {
            try ps.run()
            ps.waitUntilExit()
            wineRunning = ps.terminationStatus == 0
        } catch {
            wineRunning = false
        }
    }

    private func launchSteam(bottle: Bottle) {
        let steamExe = bottle.path + "/drive_c/Program Files (x86)/Steam/steam.exe"
        _ = CauldronBridge.shared.launchExe(
            bottleId: bottle.id,
            exePath: steamExe,
            backend: "none"
        )
    }

    private func killAllWine() {
        Task.detached {
            let sh = Process()
            sh.executableURL = URL(fileURLWithPath: "/bin/sh")
            sh.arguments = ["-c", """
                # wineserver -k needs WINEPREFIX to find the right server socket
                WINE="$HOME/Library/Cauldron/wine/bin/wineserver"
                # Try each bottle prefix
                for prefix in "$HOME/Library/Application Support/CrossOver/Bottles"/*; do
                    if [ -d "$prefix" ]; then
                        WINEPREFIX="$prefix" "$WINE" -k 2>/dev/null
                    fi
                done
                # Wait for clean exit
                "$WINE" -w 2>/dev/null
                sleep 1
                # Force kill if still alive
                if pgrep -f wineserver >/dev/null 2>&1; then
                    for prefix in "$HOME/Library/Application Support/CrossOver/Bottles"/*; do
                        if [ -d "$prefix" ]; then
                            WINEPREFIX="$prefix" "$WINE" -k9 2>/dev/null
                        fi
                    done
                    sleep 1
                fi
                true
                """]
            try? sh.run()
            sh.waitUntilExit()

            await MainActor.run {
                wineRunning = false
                Task {
                    try? await Task.sleep(for: .seconds(1))
                    checkWineRunning()
                }
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

/// Steam logo drawn as a SwiftUI Shape from the official SVG path data.
private struct SteamLogoShape: Shape {
    func path(in rect: CGRect) -> Path {
        let s = min(rect.width, rect.height) / 24.0
        var p = Path()
        // Outer circle + joystick shape from official Steam logo
        // Main body
        p.move(to: CGPoint(x: rect.minX + 11.979 * s, y: rect.minY))
        p.addCurve(
            to: CGPoint(x: rect.minX + 0.022 * s, y: rect.minY + 10.958 * s),
            control1: CGPoint(x: rect.minX + 5.678 * s, y: rect.minY),
            control2: CGPoint(x: rect.minX + 0.511 * s, y: rect.minY + 4.86 * s)
        )
        p.addLine(to: CGPoint(x: rect.minX + 6.454 * s, y: rect.minY + 13.616 * s))
        // Small circle area (joystick base)
        p.addCurve(
            to: CGPoint(x: rect.minX + 8.366 * s, y: rect.minY + 13.028 * s),
            control1: CGPoint(x: rect.minX + 6.98 * s, y: rect.minY + 13.19 * s),
            control2: CGPoint(x: rect.minX + 7.65 * s, y: rect.minY + 13.028 * s)
        )
        p.addLine(to: CGPoint(x: rect.minX + 11.414 * s, y: rect.minY + 8.892 * s))
        // Large circle (joystick top)
        p.addCurve(
            to: CGPoint(x: rect.minX + 15.944 * s, y: rect.minY + 4.314 * s),
            control1: CGPoint(x: rect.minX + 11.414 * s, y: rect.minY + 6.365 * s),
            control2: CGPoint(x: rect.minX + 13.441 * s, y: rect.minY + 4.314 * s)
        )
        p.addCurve(
            to: CGPoint(x: rect.minX + 20.475 * s, y: rect.minY + 8.845 * s),
            control1: CGPoint(x: rect.minX + 18.447 * s, y: rect.minY + 4.314 * s),
            control2: CGPoint(x: rect.minX + 20.475 * s, y: rect.minY + 6.342 * s)
        )
        p.addCurve(
            to: CGPoint(x: rect.minX + 15.944 * s, y: rect.minY + 13.376 * s),
            control1: CGPoint(x: rect.minX + 20.475 * s, y: rect.minY + 11.348 * s),
            control2: CGPoint(x: rect.minX + 18.447 * s, y: rect.minY + 13.376 * s)
        )
        p.addLine(to: CGPoint(x: rect.minX + 11.87 * s, y: rect.minY + 16.286 * s))
        // Bottom circle
        p.addCurve(
            to: CGPoint(x: rect.minX + 8.467 * s, y: rect.minY + 19.819 * s),
            control1: CGPoint(x: rect.minX + 11.873 * s, y: rect.minY + 18.164 * s),
            control2: CGPoint(x: rect.minX + 10.346 * s, y: rect.minY + 19.819 * s)
        )
        p.addCurve(
            to: CGPoint(x: rect.minX + 5.1 * s, y: rect.minY + 16.884 * s),
            control1: CGPoint(x: rect.minX + 6.591 * s, y: rect.minY + 19.819 * s),
            control2: CGPoint(x: rect.minX + 5.119 * s, y: rect.minY + 18.558 * s)
        )
        p.addLine(to: CGPoint(x: rect.minX + 0.335 * s, y: rect.minY + 14.715 * s))
        // Close the outer circle
        p.addCurve(
            to: CGPoint(x: rect.minX + 11.979 * s, y: rect.minY + 24.0 * s),
            control1: CGPoint(x: rect.minX + 1.453 * s, y: rect.minY + 19.803 * s),
            control2: CGPoint(x: rect.minX + 6.32 * s, y: rect.minY + 24.0 * s)
        )
        p.addCurve(
            to: CGPoint(x: rect.minX + 23.979 * s, y: rect.minY + 12.0 * s),
            control1: CGPoint(x: rect.minX + 18.606 * s, y: rect.minY + 24.0 * s),
            control2: CGPoint(x: rect.minX + 23.979 * s, y: rect.minY + 18.627 * s)
        )
        p.addCurve(
            to: CGPoint(x: rect.minX + 11.979 * s, y: rect.minY),
            control1: CGPoint(x: rect.minX + 23.979 * s, y: rect.minY + 5.373 * s),
            control2: CGPoint(x: rect.minX + 18.607 * s, y: rect.minY)
        )
        p.closeSubpath()
        return p
    }
}
