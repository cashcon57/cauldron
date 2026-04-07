import SwiftUI

struct CreateBottleView: View {
    @Environment(BottleListViewModel.self) private var viewModel
    @Environment(\.dismiss) private var dismiss

    @State private var name: String = ""
    @State private var selectedWineVersion: String = ""
    @State private var graphicsBackend: GraphicsBackend = .auto
    @State private var wineVersions: [WineVersionInfo] = []

    var body: some View {
        VStack(spacing: 20) {
            Text("New Bottle")
                .font(.title2.bold())
                .padding(.top, 8)

            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Name")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    TextField("My Game Bottle", text: $name)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 6) {
                    Text("Wine Version")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Picker("Wine Version", selection: $selectedWineVersion) {
                        ForEach(groupedVersions, id: \.label) { group in
                            Section(group.label) {
                                ForEach(group.versions) { version in
                                    HStack {
                                        Text(version.displayLabel)
                                        if version.installed {
                                            Text("installed")
                                                .font(.caption2)
                                                .foregroundStyle(.green)
                                        }
                                    }
                                    .tag(version.version)
                                }
                            }
                        }
                    }
                    .labelsHidden()
                }

                VStack(alignment: .leading, spacing: 6) {
                    Text("Graphics Backend")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Picker("Graphics Backend", selection: $graphicsBackend) {
                        ForEach(GraphicsBackend.allCases, id: \.self) { backend in
                            Text(backend.displayName).tag(backend)
                        }
                    }
                    .labelsHidden()
                }
            }
            .padding(.horizontal, 24)

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
                        viewModel.createBottle(name: name, wineVersion: selectedWineVersion)
                        dismiss()
                    } label: {
                        Text("Create")
                            .padding(.horizontal, 20)
                            .padding(.vertical, 8)
                    }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.defaultAction)
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                    .glassEffect(.regular.tint(.accentColor).interactive(), in: .capsule)
                }
            }
            .padding(.bottom, 16)
        }
        .frame(width: 420, height: 380)
        .onAppear {
            wineVersions = CauldronBridge.shared.getWineVersions()
            // Default to the latest stable, or first available
            if let firstStable = wineVersions.first(where: { $0.category == "stable" }) {
                selectedWineVersion = firstStable.version
            } else if let first = wineVersions.first {
                selectedWineVersion = first.version
            }
        }
    }

    // MARK: - Grouped Versions

    private struct VersionGroup {
        let label: String
        let versions: [WineVersionInfo]
    }

    private var groupedVersions: [VersionGroup] {
        let categories = ["stable", "staging", "development", "gptk"]
        let labels = ["Stable": "stable", "Staging": "staging", "Development": "development", "GPTK": "gptk"]
        return labels.sorted(by: { categoryOrder($0.value) < categoryOrder($1.value) }).compactMap { label, cat in
            let versions = wineVersions.filter { $0.category == cat }
            return versions.isEmpty ? nil : VersionGroup(label: label, versions: versions)
        }
    }

    private func categoryOrder(_ cat: String) -> Int {
        switch cat {
        case "stable": return 0
        case "staging": return 1
        case "development": return 2
        case "gptk": return 3
        default: return 4
        }
    }
}

extension WineVersionInfo {
    var displayLabel: String {
        "Wine \(version)"
    }
}
