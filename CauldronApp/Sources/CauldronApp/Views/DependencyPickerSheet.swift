import SwiftUI

struct DependencyPickerSheet: View {
    let bottleId: String
    @Binding var installingId: String?
    @Environment(\.dismiss) private var dismiss
    @State private var dependencies: [CauldronBridge.DependencyInfo] = []
    @State private var results: [String: Bool] = [:] // dep id → success
    @State private var error: String?

    var body: some View {
        VStack(spacing: 16) {
            HStack {
                Text("Install Dependencies")
                    .font(.title3.bold())
                Spacer()
                Button { dismiss() } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }

            Text("Common Windows runtimes needed by games. Requires winetricks.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)

            ScrollView {
                VStack(spacing: 0) {
                    let grouped = Dictionary(grouping: dependencies, by: \.category)
                    ForEach(grouped.keys.sorted(), id: \.self) { category in
                        Section {
                            ForEach(grouped[category] ?? [], id: \.id) { dep in
                                dependencyRow(dep)
                                if dep.id != grouped[category]?.last?.id {
                                    Divider().padding(.leading, 32)
                                }
                            }
                        } header: {
                            Text(category)
                                .font(.caption.weight(.semibold))
                                .foregroundStyle(.secondary)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.top, 12)
                                .padding(.bottom, 4)
                        }
                    }
                }
            }
            .frame(maxHeight: 400)

            if let error {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
            }
        }
        .padding(20)
        .frame(width: 440, height: 520)
        .onAppear {
            dependencies = CauldronBridge.shared.listDependencies()
        }
    }

    private func dependencyRow(_ dep: CauldronBridge.DependencyInfo) -> some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(dep.name)
                        .font(.subheadline.weight(.medium))
                    if dep.recommended {
                        Text("recommended")
                            .font(.caption2)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 1)
                            .background(.blue.opacity(0.15))
                            .foregroundStyle(.blue)
                            .clipShape(Capsule())
                    }
                }
                Text(dep.description)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }

            Spacer()

            if let success = results[dep.id] {
                Image(systemName: success ? "checkmark.circle.fill" : "xmark.circle.fill")
                    .foregroundStyle(success ? .green : .red)
            } else if installingId == dep.id {
                ProgressView().controlSize(.small)
            } else {
                Button("Install") {
                    installDep(dep)
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(installingId != nil)
            }
        }
        .padding(.vertical, 6)
    }

    private func installDep(_ dep: CauldronBridge.DependencyInfo) {
        installingId = dep.id
        error = nil
        let bridge = CauldronBridge.shared
        let bid = bottleId
        Task.detached {
            let result = bridge.installDependency(bottleId: bid, dependencyId: dep.id)
            await MainActor.run {
                installingId = nil
                if let result {
                    results[dep.id] = result.success
                    if !result.success {
                        error = result.error
                    }
                } else {
                    results[dep.id] = false
                    error = "Installation returned no result"
                }
            }
        }
    }
}
