import SwiftUI

struct BottleListView: View {
    @Environment(BottleListViewModel.self) private var viewModel

    var body: some View {
        @Bindable var vm = viewModel

        Group {
            if viewModel.bottles.isEmpty {
                ContentUnavailableView(
                    "No Bottles",
                    systemImage: "wineglass",
                    description: Text("No bottles yet. Create one to get started.")
                )
            } else {
                List(viewModel.bottles, selection: $vm.selectedBottle) { bottle in
                    BottleRow(bottle: bottle)
                        .tag(bottle)
                        .contextMenu {
                            Button(role: .destructive) {
                                viewModel.deleteBottle(bottle)
                            } label: {
                                Label("Delete", systemImage: "trash")
                            }
                        }
                        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                            Button(role: .destructive) {
                                viewModel.deleteBottle(bottle)
                            } label: {
                                Label("Delete", systemImage: "trash")
                            }
                        }
                }
            }
        }
        .navigationTitle("Bottles")
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
