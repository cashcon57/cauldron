import SwiftUI

/// Full-screen inspect view for a single patch showing all analysis data,
/// adaptation preview, and actions.
struct PatchInspectSheet: View {
    let patch: PatchEntry
    let analysis: PatchAnalysis?
    let onApply: () -> Void
    let onSkip: () -> Void
    let onReverse: () -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            // Header
            header
            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    summarySection
                    if let a = analysis {
                        if let action = a.suggestedAction, !action.isEmpty {
                            recommendationBanner(action)
                        }
                        compatibilitySection(a)
                        impactSection(a)
                        if let modding = a.moddingImpact, !modding.isEmpty {
                            moddingSection(modding)
                        }
                        if a.canAutoAdapt == true {
                            adaptationSection(a)
                        }
                        if !a.affectedGames.isEmpty {
                            affectedGamesSection(a)
                        }
                        if let rating = a.protondbRating {
                            protonDBSection(rating)
                        }
                    } else {
                        noAnalysisHint
                    }
                }
                .padding(20)
            }

            Divider()
            // Action bar
            actionBar
        }
        .frame(width: 560, height: 600)
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: 12) {
            Image(systemName: statusIcon)
                .font(.title2)
                .foregroundStyle(statusColor)

            VStack(alignment: .leading, spacing: 2) {
                Text(patch.title)
                    .font(.headline)
                    .lineLimit(2)

                HStack(spacing: 8) {
                    Text(String(patch.hash.prefix(8)))
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                    Text("by \(patch.author)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()

            Button { dismiss() } label: {
                Image(systemName: "xmark.circle.fill")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
        }
        .padding(16)
    }

    // MARK: - Summary

    private var summarySection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionHeader("Summary", icon: "doc.text")

            InfoRow(label: "Classification", value: classificationDisplayName)
            InfoRow(label: "What this means", value: classificationExplanation)
            InfoRow(label: "Portability", value: transferabilityDisplayName)
            InfoRow(label: "Status", value: patch.status.capitalized)
        }
        .padding(14)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }

    // MARK: - Compatibility

    private func compatibilitySection(_ a: PatchAnalysis) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionHeader("Compatibility Check", icon: "checkmark.shield")

            if let clean = a.appliesCleanly {
                HStack(spacing: 8) {
                    Image(systemName: clean ? "checkmark.circle.fill" : "xmark.circle.fill")
                        .foregroundStyle(clean ? .green : .red)
                        .font(.title3)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(clean ? "Applies cleanly" : "Has conflicts")
                            .font(.subheadline.weight(.medium))
                        Text(clean
                             ? "Dry-run passed — this patch can be applied to your Wine source without conflicts."
                             : "Dry-run failed — this patch conflicts with your current Wine source tree.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                if !a.conflictFiles.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Conflicting files:")
                            .font(.caption.weight(.medium))
                        ForEach(a.conflictFiles, id: \.self) { file in
                            Text("  \(file)")
                                .font(.caption.monospaced())
                                .foregroundStyle(.red)
                        }
                    }
                }
            } else {
                HStack(spacing: 8) {
                    Image(systemName: "questionmark.circle")
                        .foregroundStyle(.secondary)
                    Text("Run Analyze to check compatibility")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(14)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }

    // MARK: - Impact

    private func impactSection(_ a: PatchAnalysis) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionHeader("Impact Assessment", icon: "gauge.with.dots.needle.33percent")

            HStack(spacing: 12) {
                impactBadge(a.impact)

                VStack(alignment: .leading, spacing: 2) {
                    Text(a.impactReason)
                        .font(.subheadline)
                    Text("+\(a.linesAdded) lines added, -\(a.linesRemoved) lines removed")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            if !a.affectedDlls.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Wine components affected:")
                        .font(.caption.weight(.medium))
                    FlowLayout(spacing: 6) {
                        ForEach(a.affectedDlls, id: \.self) { dll in
                            Text(dll)
                                .font(.caption.monospaced())
                                .padding(.horizontal, 8)
                                .padding(.vertical, 3)
                                .glassEffect(.regular, in: .capsule)
                        }
                    }
                }
            }
        }
        .padding(14)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }

    // MARK: - Adaptation

    private func adaptationSection(_ a: PatchAnalysis) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionHeader("Auto-Adaptation", icon: "wand.and.stars")

            HStack(spacing: 8) {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(.cyan)
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(a.adaptationTransformCount ?? 0) Linux→macOS transforms available")
                        .font(.subheadline.weight(.medium))
                    Text("Confidence: \(a.adaptationConfidence ?? "unknown")")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Text("When you click Apply, Cauldron will automatically replace Linux-specific APIs with their macOS equivalents before applying the patch.")
                .font(.caption)
                .foregroundStyle(.secondary)

            if let warnings = a.adaptationWarnings, !warnings.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Manual review needed for:")
                        .font(.caption.weight(.medium))
                        .foregroundStyle(.orange)
                    ForEach(warnings, id: \.self) { warning in
                        HStack(alignment: .top, spacing: 4) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .font(.caption2)
                                .foregroundStyle(.orange)
                            Text(warning)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .padding(14)
        .glassEffect(.regular.tint(.cyan), in: .rect(cornerRadius: 12))
    }

    // MARK: - Affected Games

    private func affectedGamesSection(_ a: PatchAnalysis) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionHeader("Affected Games", icon: "gamecontroller")

            Text("This patch modifies DLLs that these installed games depend on:")
                .font(.caption)
                .foregroundStyle(.secondary)

            ForEach(a.affectedGames, id: \.self) { game in
                HStack(spacing: 8) {
                    Image(systemName: "circle.fill")
                        .font(.system(size: 6))
                        .foregroundStyle(.blue)
                    Text(game)
                        .font(.subheadline)
                }
            }
        }
        .padding(14)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }

    // MARK: - ProtonDB

    private func protonDBSection(_ rating: String) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionHeader("Community Data", icon: "person.3")

            HStack(spacing: 8) {
                Text(rating.capitalized)
                    .font(.subheadline.weight(.bold))
                    .foregroundStyle(protondbColor(rating))
                Text("on ProtonDB")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            Text("Community-reported compatibility rating for the game this patch targets.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(14)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }

    // MARK: - No Analysis Hint

    private var noAnalysisHint: some View {
        VStack(spacing: 8) {
            Image(systemName: "stethoscope")
                .font(.title)
                .foregroundStyle(.secondary)
            Text("No analysis data yet")
                .font(.subheadline.weight(.medium))
            Text("Click the Analyze button to run compatibility checks, impact scoring, and auto-adaptation detection for all patches.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 30)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }

    // MARK: - Recommendation Banner

    private func recommendationBanner(_ action: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: action.lowercased().contains("safe") ? "checkmark.seal.fill" : action.lowercased().contains("skip") ? "forward.fill" : "eye.fill")
                .foregroundStyle(action.lowercased().contains("safe") ? .green : action.lowercased().contains("skip") ? .gray : .yellow)
                .font(.title3)
            VStack(alignment: .leading, spacing: 2) {
                Text("Recommendation")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                Text(action)
                    .font(.subheadline)
            }
            Spacer()
        }
        .padding(14)
        .glassEffect(.regular.tint(action.lowercased().contains("safe") ? .green : action.lowercased().contains("skip") ? .gray : .yellow), in: .rect(cornerRadius: 12))
    }

    // MARK: - Modding Impact

    private func moddingSection(_ impacts: [String]) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionHeader("Modding & Gaming Impact", icon: "puzzlepiece.extension")

            ForEach(impacts, id: \.self) { impact in
                HStack(alignment: .top, spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.caption)
                        .foregroundStyle(.yellow)
                    Text(impact)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(14)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }

    // MARK: - Action Bar

    private var actionBar: some View {
        HStack(spacing: 12) {
            if patch.status == "pending" {
                Button(action: { onApply(); dismiss() }) {
                    Label(analysis?.canAutoAdapt == true ? "Auto-Adapt & Apply" : "Apply Patch", systemImage: "checkmark.circle")
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 8)
                }
                .buttonStyle(.plain)
                .glassEffect(.regular.tint(.green).interactive(), in: .capsule)

                Button(action: { onSkip(); dismiss() }) {
                    Label("Skip", systemImage: "forward")
                        .padding(.horizontal, 20)
                        .padding(.vertical, 8)
                }
                .buttonStyle(.plain)
                .glassEffect(.regular.interactive(), in: .capsule)
            } else if patch.status == "applied" {
                Button(action: { onReverse(); dismiss() }) {
                    Label("Reverse Patch", systemImage: "arrow.uturn.backward")
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 8)
                }
                .buttonStyle(.plain)
                .glassEffect(.regular.tint(.orange).interactive(), in: .capsule)
            } else {
                Text("This patch has been skipped")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity)
            }
        }
        .padding(16)
    }

    // MARK: - Helpers

    private func sectionHeader(_ title: String, icon: String) -> some View {
        Label(title, systemImage: icon)
            .font(.subheadline.weight(.semibold))
    }

    private func impactBadge(_ impact: String) -> some View {
        Text(impact.localizedCapitalized)
            .font(.caption.weight(.bold))
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .background(impactColor(impact).opacity(0.2))
            .foregroundStyle(impactColor(impact))
            .clipShape(Capsule())
    }

    private func impactColor(_ impact: String) -> Color {
        PatchDisplayHelpers.impactColor(impact)
    }

    private func protondbColor(_ rating: String) -> Color {
        PatchDisplayHelpers.protondbColor(rating)
    }

    private var statusIcon: String {
        PatchDisplayHelpers.statusIcon(patch.status)
    }

    private var statusColor: Color {
        PatchDisplayHelpers.statusColor(patch.status)
    }

    private var classificationDisplayName: String {
        PatchDisplayHelpers.classificationDisplayName(patch.classification)
    }

    private var classificationExplanation: String {
        switch patch.classification {
        case "WineApiFix": return "Changes to Wine's core DLLs or server. Usually portable to macOS."
        case "DxvkFix": return "DXVK translates DirectX 9/10/11 → Vulkan. Works via MoltenVK on macOS."
        case "Vkd3dFix": return "VKD3D-Proton translates DirectX 12 → Vulkan. May need Metal adjustments."
        case "GameConfig": return "Game-specific settings or compatibility tweaks."
        case "KernelWorkaround": return "Uses Linux kernel features. Needs macOS equivalent (MSync, kqueue, etc.)."
        case "SteamIntegration": return "Steam client or VR runtime integration. May need macOS path fixes."
        case "BuildSystem": return "Build infrastructure change. No runtime impact."
        default: return "Could not determine patch type. Manual review recommended."
        }
    }

    private var transferabilityDisplayName: String {
        switch patch.transferability {
        case "High": return "\(PatchDisplayHelpers.transferabilityLabel("High")) — can apply directly"
        case "Medium": return "\(PatchDisplayHelpers.transferabilityLabel("Medium")) — may need adjustments"
        case "Low": return "\(PatchDisplayHelpers.transferabilityLabel("Low")) — requires macOS-specific changes"
        case "None": return "\(PatchDisplayHelpers.transferabilityLabel("None")) — infrastructure only"
        default: return patch.transferability
        }
    }
}

// MARK: - Info Row

private struct InfoRow: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(value)
                .font(.subheadline)
        }
    }
}

// MARK: - Flow Layout (for DLL tags)

private struct FlowLayout: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let result = arrange(proposal: proposal, subviews: subviews)
        return result.size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = arrange(proposal: proposal, subviews: subviews)
        for (index, position) in result.positions.enumerated() {
            subviews[index].place(at: CGPoint(x: bounds.minX + position.x, y: bounds.minY + position.y), proposal: .unspecified)
        }
    }

    private func arrange(proposal: ProposedViewSize, subviews: Subviews) -> (size: CGSize, positions: [CGPoint]) {
        let maxWidth = proposal.width ?? .infinity
        var positions: [CGPoint] = []
        var x: CGFloat = 0
        var y: CGFloat = 0
        var maxHeight: CGFloat = 0
        var rowHeight: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x + size.width > maxWidth && x > 0 {
                x = 0
                y += rowHeight + spacing
                rowHeight = 0
            }
            positions.append(CGPoint(x: x, y: y))
            rowHeight = max(rowHeight, size.height)
            x += size.width + spacing
            maxHeight = max(maxHeight, y + rowHeight)
        }

        return (CGSize(width: maxWidth, height: maxHeight), positions)
    }
}
