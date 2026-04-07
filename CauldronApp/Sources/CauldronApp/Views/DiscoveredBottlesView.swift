import SwiftUI

struct DiscoveredBottlesView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var discoveredBottles: [DiscoveredBottle] = []
    @State private var isScanning = true
    @State private var importedPaths: Set<String> = []
    @State private var importError: String?

    var body: some View {
        VStack(spacing: 0) {
            // Header
            VStack(spacing: 8) {
                if allImported && !discoveredBottles.isEmpty {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 40))
                        .foregroundStyle(.green)
                        .symbolRenderingMode(.hierarchical)

                    Text("Import Successful")
                        .font(.title2.bold())

                    Text("\(discoveredBottles.count) bottle\(discoveredBottles.count == 1 ? "" : "s") imported and ready to use.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 24)
                } else {
                    Image(systemName: "magnifyingglass.circle.fill")
                        .font(.system(size: 40))
                        .foregroundStyle(.blue)
                        .symbolRenderingMode(.hierarchical)

                    Text("Existing Bottles Found")
                        .font(.title2.bold())

                    Text("Cauldron found Wine bottles from other applications. You can import them to manage them here.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 24)
                }
            }
            .padding(.top, 20)
            .padding(.bottom, 12)

            // Content
            if isScanning {
                Spacer()
                ProgressView("Scanning for bottles...")
                Spacer()
            } else if discoveredBottles.isEmpty {
                Spacer()
                ContentUnavailableView(
                    "No Bottles Found",
                    systemImage: "wineglass",
                    description: Text("No existing Wine bottles were detected from other applications.")
                )
                Spacer()
            } else {
                ScrollView {
                    VStack(spacing: 10) {
                        ForEach(discoveredBottles) { bottle in
                            discoveredBottleCard(bottle)
                        }
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 8)
                }
            }

            // Footer
            HStack(spacing: 12) {
                if allImported && !discoveredBottles.isEmpty {
                    // Post-import: show success and a single "Done" button
                    Button {
                        dismiss()
                    } label: {
                        Text("Done")
                            .frame(minWidth: 80)
                            .padding(.horizontal, 20)
                            .padding(.vertical, 10)
                    }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.defaultAction)
                    .glassEffect(.regular.tint(.accentColor).interactive(), in: .capsule)
                } else {
                    Button {
                        dismiss()
                    } label: {
                        Text("Skip")
                            .frame(minWidth: 80)
                            .padding(.horizontal, 20)
                            .padding(.vertical, 10)
                    }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.cancelAction)
                    .glassEffect(.regular.interactive(), in: .capsule)

                    if !discoveredBottles.isEmpty {
                        Button {
                            importAll()
                        } label: {
                            Text("Import All")
                                .frame(minWidth: 80)
                                .padding(.horizontal, 20)
                                .padding(.vertical, 10)
                        }
                        .buttonStyle(.plain)
                        .glassEffect(.regular.tint(.accentColor).interactive(), in: .capsule)
                    }
                }
            }
            .padding(.bottom, 16)
        }
        .frame(width: 520, height: 480)
        .alert("Import Error", isPresented: Binding(
            get: { importError != nil },
            set: { if !$0 { importError = nil } }
        )) {
            Button("OK") { importError = nil }
        } message: {
            Text(importError ?? "")
        }
        .onAppear {
            scanForBottles()
        }
    }

    // MARK: - Bottle Card

    private func discoveredBottleCard(_ bottle: DiscoveredBottle) -> some View {
        HStack(spacing: 14) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    Text(bottle.name)
                        .font(.headline)
                        .lineLimit(1)

                    Text(bottle.source)
                        .font(.caption2.bold())
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .glassEffect(.regular.tint(sourceColor(for: bottle.source)), in: .capsule)
                }

                HStack(spacing: 12) {
                    Label(bottle.wineVersion, systemImage: "wineglass")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    Label(formattedSize(bottle.sizeBytes), systemImage: "internaldrive")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    if bottle.gameCount > 0 {
                        Label("\(bottle.gameCount) game\(bottle.gameCount == 1 ? "" : "s")", systemImage: "gamecontroller")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                if bottle.hasSteam {
                    HStack(spacing: 4) {
                        Image(systemName: "checkmark.seal.fill")
                            .foregroundStyle(.blue)
                        Text("Has Steam")
                            .font(.caption2)
                            .foregroundStyle(.blue)
                    }
                }
            }

            Spacer()

            if importedPaths.contains(bottle.path) {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(.green)
                    .font(.title3)
            } else {
                Button("Import") {
                    importBottle(bottle)
                }
                .buttonStyle(.plain)
                .padding(.horizontal, 14)
                .padding(.vertical, 6)
                .glassEffect(.regular.tint(.accentColor).interactive(), in: .capsule)
            }
        }
        .padding(14)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }

    // MARK: - Helpers

    private func sourceColor(for source: String) -> Color {
        switch source.lowercased() {
        case "whisky":
            return .orange
        case "crossover":
            return .purple
        case "wineskin":
            return .brown
        case "standalone wine":
            return .gray
        case "cauldron":
            return .blue
        default:
            return .secondary
        }
    }

    private func formattedSize(_ bytes: Int64) -> String {
        let gb = Double(bytes) / (1024 * 1024 * 1024)
        if gb >= 1.0 {
            return String(format: "%.1f GB", gb)
        }
        let mb = Double(bytes) / (1024 * 1024)
        return String(format: "%.0f MB", mb)
    }

    private var allImported: Bool {
        discoveredBottles.allSatisfy { importedPaths.contains($0.path) }
    }

    // MARK: - Actions

    private func scanForBottles() {
        isScanning = true
        Task {
            let bottles = CauldronBridge.shared.discoverBottles()
            discoveredBottles = bottles
            isScanning = false
        }
    }

    private func debugLog(_ msg: String) {
        let path = "/tmp/cauldron_bridge_debug.log"
        let existing = (try? String(contentsOfFile: path, encoding: .utf8)) ?? ""
        try? (existing + msg + "\n").write(toFile: path, atomically: true, encoding: .utf8)
    }

    private func importBottle(_ bottle: DiscoveredBottle) {
        debugLog("VIEW importBottle called: \(bottle.name) path=\(bottle.path)")
        let result = CauldronBridge.shared.importBottle(
            sourcePath: bottle.path,
            name: bottle.name
        )
        if result != nil {
            _ = withAnimation(.easeInOut(duration: 0.3)) {
                importedPaths.insert(bottle.path)
            }
        } else {
            importError = "Failed to import \(bottle.name)"
        }
    }

    private func importAll() {
        debugLog("VIEW importAll called: \(discoveredBottles.count) bottles, \(importedPaths.count) already imported")
        for bottle in discoveredBottles where !importedPaths.contains(bottle.path) {
            let result = CauldronBridge.shared.importBottle(
                sourcePath: bottle.path,
                name: bottle.name
            )
            if result != nil {
                _ = withAnimation(.easeInOut(duration: 0.3)) {
                    importedPaths.insert(bottle.path)
                }
            } else {
                importError = "Failed to import \(bottle.name)"
            }
        }
    }

    // MARK: - Discovery (runs off main thread)

    private static func discoverBottles() -> [DiscoveredBottle] {
        var results: [DiscoveredBottle] = []
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser

        // Check CrossOver bottles
        let crossoverDir = home
            .appendingPathComponent("Library/Application Support/CrossOver/Bottles")
        if let entries = try? fm.contentsOfDirectory(atPath: crossoverDir.path) {
            for entry in entries {
                let bottlePath = crossoverDir.appendingPathComponent(entry)
                var isDir: ObjCBool = false
                guard fm.fileExists(atPath: bottlePath.path, isDirectory: &isDir),
                      isDir.boolValue else { continue }
                let hasSteam = fm.fileExists(
                    atPath: bottlePath.appendingPathComponent(
                        "drive_c/Program Files (x86)/Steam/steam.exe"
                    ).path
                )
                results.append(DiscoveredBottle(
                    name: entry,
                    path: bottlePath.path,
                    source: "CrossOver",
                    wineVersion: "CrossOver",
                    sizeBytes: 0,
                    hasSteam: hasSteam,
                    gameCount: 0,
                    graphicsBackend: "unknown"
                ))
            }
        }

        // Check Whisky bottles
        let whiskyContainers = [
            "Library/Containers/com.isaacmarovitz.Whisky/Bottles",
            "Library/Application Support/Whisky/Bottles",
        ]
        for containerRelPath in whiskyContainers {
            let dir = home.appendingPathComponent(containerRelPath)
            guard let entries = try? fm.contentsOfDirectory(atPath: dir.path) else { continue }
            for entry in entries {
                let bottlePath = dir.appendingPathComponent(entry)
                var isDir: ObjCBool = false
                guard fm.fileExists(atPath: bottlePath.path, isDirectory: &isDir),
                      isDir.boolValue else { continue }
                let hasSteam = fm.fileExists(
                    atPath: bottlePath.appendingPathComponent(
                        "drive_c/Program Files (x86)/Steam/steam.exe"
                    ).path
                )
                results.append(DiscoveredBottle(
                    name: entry,
                    path: bottlePath.path,
                    source: "Whisky",
                    wineVersion: "unknown",
                    sizeBytes: 0,
                    hasSteam: hasSteam,
                    gameCount: 0,
                    graphicsBackend: "unknown"
                ))
            }
        }

        // Check default Wine prefix
        let defaultPrefix = home.appendingPathComponent(".wine")
        if fm.fileExists(atPath: defaultPrefix.appendingPathComponent("drive_c").path) {
            let hasSteam = fm.fileExists(
                atPath: defaultPrefix.appendingPathComponent(
                    "drive_c/Program Files (x86)/Steam/steam.exe"
                ).path
            )
            results.append(DiscoveredBottle(
                name: "Default Wine Prefix",
                path: defaultPrefix.path,
                source: "Standalone Wine",
                wineVersion: "unknown",
                sizeBytes: 0,
                hasSteam: hasSteam,
                gameCount: 0,
                graphicsBackend: "unknown"
            ))
        }

        return results
    }
}
