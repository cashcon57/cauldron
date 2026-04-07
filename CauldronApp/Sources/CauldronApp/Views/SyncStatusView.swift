import SwiftUI

struct SyncStatusView: View {
    @State private var syncStatus: SyncStatus?
    @State private var isSyncing: Bool = false
    @State private var selectedCategory: PatchCategory? = nil
    @State private var patches: [PatchEntry] = []
    @State private var actionInProgress: String? = nil
    @State private var actionError: String? = nil
    @State private var analyses: [String: PatchAnalysis] = [:]
    @State private var isAnalyzing: Bool = false
    @State private var isVerifyingBuild: Bool = false
    @State private var buildResult: String? = nil
    @State private var inspectingPatch: PatchEntry? = nil

    var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                statusBar
                explanationCard
                patchOverviewSection

                if let category = selectedCategory {
                    patchDetailSection(for: category)
                }

                errorSection
                actionErrorBanner
                buildResultBanner
                actionButtons
            }
            .padding()
        }
        .navigationTitle("Patches")
        .onAppear { loadStatus() }
        .onChange(of: selectedCategory) { _, _ in
            loadPatches()
        }
        .sheet(item: $inspectingPatch) { patch in
            PatchInspectSheet(
                patch: patch,
                analysis: analyses[patch.hash],
                onApply: { performPatchAction(hash: patch.hash, action: "apply") },
                onSkip: { performPatchAction(hash: patch.hash, action: "skip") },
                onReverse: { performPatchAction(hash: patch.hash, action: "reverse") }
            )
        }
    }

    // MARK: - Compact Status Bar

    private var statusBar: some View {
        HStack(spacing: 12) {
            Image(systemName: "arrow.triangle.2.circlepath")
                .font(.title3)
                .foregroundStyle(.tint)

            VStack(alignment: .leading, spacing: 2) {
                Text("Proton Patch Sync")
                    .font(.headline)

                if let status = syncStatus {
                    if let timestamp = status.lastSyncTimestamp, !timestamp.isEmpty {
                        Text("Last sync: \(formattedTimestamp(timestamp))")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    } else {
                        Text("Never synced")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                } else {
                    Text("Waiting for first sync...")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()

            if let status = syncStatus, let hash = status.lastCommitHash, !hash.isEmpty {
                Text(String(hash.prefix(8)))
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }

    // MARK: - Explanation Card

    private var explanationCard: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("How patch syncing works")
                .font(.subheadline.weight(.semibold))

            Text("Cauldron monitors Valve's Proton and CodeWeavers' CrossOver repositories for Wine patches that improve game compatibility on macOS. Each patch is classified by source and can be individually applied or skipped. Applied patches modify your local Wine source — they can be reversed at any time.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
    }

    // MARK: - Patch Overview

    private var patchOverviewSection: some View {
        let status = syncStatus ?? SyncStatus(
            lastSyncTimestamp: nil,
            lastCommitHash: nil,
            totalCommitsProcessed: 0,
            commitsApplied: 0,
            commitsPending: 0,
            commitsSkipped: 0,
            lastError: nil
        )

        let total = max(status.totalCommitsProcessed, 1)
        let appliedFraction = Double(status.commitsApplied) / Double(total)
        let pendingFraction = Double(status.commitsPending) / Double(total)
        let skippedFraction = Double(status.commitsSkipped) / Double(total)

        return VStack(spacing: 12) {
            // Progress bar
            GeometryReader { geometry in
                HStack(spacing: 2) {
                    if status.commitsApplied > 0 {
                        Rectangle()
                            .fill(Color.green.opacity(0.8))
                            .frame(width: max(geometry.size.width * appliedFraction, 0))
                    }
                    if status.commitsPending > 0 {
                        Rectangle()
                            .fill(Color.orange.opacity(0.8))
                            .frame(width: max(geometry.size.width * pendingFraction, 0))
                    }
                    if status.commitsSkipped > 0 {
                        Rectangle()
                            .fill(Color.gray.opacity(0.5))
                            .frame(width: max(geometry.size.width * skippedFraction, 0))
                    }
                    if status.totalCommitsProcessed == 0 {
                        Rectangle()
                            .fill(Color.gray.opacity(0.2))
                    }
                    Spacer(minLength: 0)
                }
                .clipShape(RoundedRectangle(cornerRadius: 4))
            }
            .frame(height: 8)

            HStack(spacing: 12) {
                PatchStatCard(
                    label: "Total",
                    value: "\(status.totalCommitsProcessed)",
                    icon: "number",
                    tint: .accentColor,
                    isSelected: selectedCategory == .total
                ) {
                    withAnimation(.snappy) {
                        selectedCategory = (selectedCategory == .total) ? nil : .total
                    }
                }
                PatchStatCard(
                    label: "Applied",
                    value: "\(status.commitsApplied)",
                    icon: "checkmark.circle.fill",
                    tint: .green,
                    isSelected: selectedCategory == .applied
                ) {
                    withAnimation(.snappy) {
                        selectedCategory = (selectedCategory == .applied) ? nil : .applied
                    }
                }
                PatchStatCard(
                    label: "Pending",
                    value: "\(status.commitsPending)",
                    icon: "clock.fill",
                    tint: .orange,
                    isSelected: selectedCategory == .pending
                ) {
                    withAnimation(.snappy) {
                        selectedCategory = (selectedCategory == .pending) ? nil : .pending
                    }
                }
                PatchStatCard(
                    label: "Skipped",
                    value: "\(status.commitsSkipped)",
                    icon: "arrow.uturn.right",
                    tint: .gray,
                    isSelected: selectedCategory == .skipped
                ) {
                    withAnimation(.snappy) {
                        selectedCategory = (selectedCategory == .skipped) ? nil : .skipped
                    }
                }
            }
        }
    }

    // MARK: - Patch Detail Drill-Down

    @ViewBuilder
    private func patchDetailSection(for category: PatchCategory) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Image(systemName: category.icon)
                    .foregroundStyle(category.tint)
                Text(category.title)
                    .font(.headline)
                Spacer()
                Button {
                    withAnimation(.snappy) { selectedCategory = nil }
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }

            Text(category.description)
                .font(.caption)
                .foregroundStyle(.secondary)

            Divider()

            if patches.isEmpty {
                VStack(spacing: 8) {
                    Image(systemName: "tray")
                        .font(.title2)
                        .foregroundStyle(.secondary)
                    Text("No patches in this category yet.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Text("Run a sync to discover new Proton patches.")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 20)
            } else {
                groupedPatchList
            }
        }
        .padding(16)
        .glassEffect(.regular, in: .rect(cornerRadius: 12))
        .transition(.opacity.combined(with: .move(edge: .top)))
    }

    // MARK: - Error Section

    @ViewBuilder
    private var errorSection: some View {
        if let status = syncStatus, let error = status.lastError, !error.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Label("Last Error", systemImage: "exclamationmark.triangle.fill")
                    .font(.headline)
                    .foregroundStyle(.red)

                Text(error)
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .glassEffect(.regular.tint(.red), in: .rect(cornerRadius: 12))
        }
    }

    // MARK: - Action Error Banner

    @ViewBuilder
    private var actionErrorBanner: some View {
        if let error = actionError {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.orange)
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Button {
                    actionError = nil
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }
            .padding(12)
            .glassEffect(.regular.tint(.orange), in: .rect(cornerRadius: 10))
        }
    }

    // MARK: - Build Result Banner

    @ViewBuilder
    private var buildResultBanner: some View {
        if let result = buildResult {
            HStack(spacing: 8) {
                Image(systemName: result.hasPrefix("Build succeeded") ? "checkmark.circle.fill" : "xmark.circle.fill")
                    .foregroundStyle(result.hasPrefix("Build succeeded") ? .green : .red)
                Text(result)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(5)
                Spacer()
                Button {
                    buildResult = nil
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }
            .padding(12)
            .glassEffect(.regular, in: .rect(cornerRadius: 10))
        }
    }

    // MARK: - Action Buttons

    private var actionButtons: some View {
        HStack(spacing: 12) {
            // Sync
            Button {
                withAnimation { isSyncing = true }
                let bridge = CauldronBridge.shared
                Task.detached {
                    let result = bridge.runSync()
                    await MainActor.run {
                        if let status = result {
                            syncStatus = status
                        } else {
                            loadStatus()
                        }
                        withAnimation { isSyncing = false }
                    }
                }
            } label: {
                HStack(spacing: 6) {
                    if isSyncing {
                        ProgressView().controlSize(.small)
                    } else {
                        Image(systemName: "arrow.clockwise")
                    }
                    Text(isSyncing ? "Syncing..." : "Sync")
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .glassEffect(.regular.tint(.blue).interactive(), in: .capsule)
            .disabled(isSyncing || isAnalyzing)

            // Analyze
            Button {
                withAnimation { isAnalyzing = true }
                let bridge = CauldronBridge.shared
                Task.detached {
                    let results = bridge.analyzePatches()
                    await MainActor.run {
                        var map: [String: PatchAnalysis] = [:]
                        for a in results {
                            map[a.hash] = a
                        }
                        analyses = map
                        withAnimation { isAnalyzing = false }
                    }
                }
            } label: {
                HStack(spacing: 6) {
                    if isAnalyzing {
                        ProgressView().controlSize(.small)
                    } else {
                        Image(systemName: "stethoscope")
                    }
                    Text(isAnalyzing ? "Analyzing..." : "Analyze")
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .glassEffect(.regular.tint(.purple).interactive(), in: .capsule)
            .disabled(isSyncing || isAnalyzing)
            .help("Run dry-run checks, impact scoring, and game compatibility analysis on all patches")

            // Verify Build
            Button {
                withAnimation { isVerifyingBuild = true }
                buildResult = nil
                let bridge = CauldronBridge.shared
                Task.detached {
                    let result = bridge.verifyBuild()
                    await MainActor.run {
                        if let result {
                            buildResult = result.success ? "Build succeeded." : (result.error ?? "Build failed.")
                        }
                        withAnimation { isVerifyingBuild = false }
                    }
                }
            } label: {
                HStack(spacing: 6) {
                    if isVerifyingBuild {
                        ProgressView().controlSize(.small)
                    } else {
                        Image(systemName: "hammer")
                    }
                    Text(isVerifyingBuild ? "Building..." : "Verify Build")
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .glassEffect(.regular.tint(.orange).interactive(), in: .capsule)
            .disabled(isVerifyingBuild)
            .help("Compile Wine source from scratch to verify all applied patches produce a working build. This runs ./configure && make and takes 10-30 minutes.")
        }
    }

    // MARK: - Data Loading

    private func loadStatus() {
        syncStatus = CauldronBridge.shared.getSyncStatus()
    }

    // MARK: - Helpers

    private func formattedTimestamp(_ raw: String) -> String {
        if let seconds = Double(raw) {
            let date = Date(timeIntervalSince1970: seconds)
            let formatter = RelativeDateTimeFormatter()
            formatter.unitsStyle = .abbreviated
            return formatter.localizedString(for: date, relativeTo: Date())
        }
        return raw
    }

    // MARK: - Grouped Patch List

    /// Groups patches by classification, then by transferability within each group.
    /// Similar component updates (e.g. multiple "Update Wine Mono") are nested.
    private var groupedPatchList: some View {
        let grouped = groupPatches(patches)
        return ForEach(grouped, id: \.key) { group in
            VStack(alignment: .leading, spacing: 4) {
                // Classification header
                HStack(spacing: 6) {
                    Text(classificationDisplayName(group.key))
                        .font(.subheadline.weight(.semibold))
                    Text("\(group.patches.count)")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 1)
                        .background(Color.secondary.opacity(0.15))
                        .clipShape(Capsule())
                }
                .padding(.top, 8)

                // Within each classification, group by transferability
                let byTransfer = Dictionary(grouping: group.patches, by: \.transferability)
                let transferOrder = ["High", "Medium", "Low", "None"]
                ForEach(transferOrder, id: \.self) { level in
                    if let levelPatches = byTransfer[level], !levelPatches.isEmpty {
                        // Nest similar component updates
                        let nested = nestSimilarPatches(levelPatches)
                        ForEach(nested, id: \.id) { entry in
                            if entry.children.isEmpty {
                                // Single patch
                                PatchRowView(patch: entry.patch, analysis: analyses[entry.patch.hash], onApply: {
                                    performPatchAction(hash: entry.patch.hash, action: "apply")
                                }, onSkip: {
                                    performPatchAction(hash: entry.patch.hash, action: "skip")
                                }, onReverse: {
                                    performPatchAction(hash: entry.patch.hash, action: "reverse")
                                }, onInspect: {
                                    inspectingPatch = entry.patch
                                })
                            } else {
                                // Grouped similar patches
                                NestedPatchGroup(entry: entry, analyses: analyses, onInspect: { patch in
                                    inspectingPatch = patch
                                }) { hash, action in
                                    performPatchAction(hash: hash, action: action)
                                }
                            }
                        }
                    }
                }
            }
            Divider()
        }
    }

    private func classificationDisplayName(_ key: String) -> String {
        PatchDisplayHelpers.classificationDisplayName(key, plural: true)
    }

    // MARK: - Grouping Helpers

    struct ClassificationGroup {
        let key: String
        let patches: [PatchEntry]
    }

    struct NestedEntry: Identifiable {
        let id: String
        let patch: PatchEntry
        let children: [PatchEntry]
        var label: String { children.isEmpty ? patch.title : "\(patch.title) (\(children.count + 1) updates)" }
    }

    private func groupPatches(_ patches: [PatchEntry]) -> [ClassificationGroup] {
        let byClass = Dictionary(grouping: patches, by: \.classification)
        let classOrder = ["WineApiFix", "DxvkFix", "Vkd3dFix", "GameConfig", "KernelWorkaround", "SteamIntegration", "BuildSystem", "Unknown"]
        return classOrder.compactMap { key in
            guard let patches = byClass[key], !patches.isEmpty else { return nil }
            return ClassificationGroup(key: key, patches: patches)
        }
    }

    /// Nest patches whose titles share a common prefix (e.g. "Update Wine Mono to X", "Update Wine Mono to Y").
    private func nestSimilarPatches(_ patches: [PatchEntry]) -> [NestedEntry] {
        var groups: [(prefix: String, items: [PatchEntry])] = []

        for patch in patches {
            let prefix = componentPrefix(from: patch.title)
            if let idx = groups.firstIndex(where: { $0.prefix == prefix }) {
                groups[idx].items.append(patch)
            } else {
                groups.append((prefix: prefix, items: [patch]))
            }
        }

        return groups.map { group in
            if group.items.count == 1 {
                return NestedEntry(id: group.items[0].hash, patch: group.items[0], children: [])
            } else {
                let first = group.items[0]
                let rest = Array(group.items.dropFirst())
                return NestedEntry(id: "group-\(group.prefix)", patch: first, children: rest)
            }
        }
    }

    /// Extract a component prefix for grouping.
    /// "Update Wine Mono to 10.4.1" → "Update Wine Mono"
    /// "proton: Disable gameinput" → "proton"
    private func componentPrefix(from title: String) -> String {
        // "update X to Y" pattern
        let lower = title.lowercased()
        if lower.hasPrefix("update ") {
            if let toRange = lower.range(of: " to ") {
                return String(title[title.startIndex..<toRange.lowerBound])
            }
            // "update X vN" pattern
            if let vRange = lower.range(of: " v", options: .backwards) {
                let afterV = lower[vRange.upperBound...]
                if afterV.first?.isNumber == true {
                    return String(title[title.startIndex..<vRange.lowerBound])
                }
            }
        }
        // Use full title as unique key (no grouping)
        return title
    }

    private func performPatchAction(hash: String, action: String) {
        actionInProgress = hash
        actionError = nil
        let bridge = CauldronBridge.shared
        Task.detached {
            let result: PatchActionResult? = switch action {
            case "apply": bridge.applyPatch(hash: hash)
            case "skip": bridge.skipPatch(hash: hash)
            case "reverse": bridge.reversePatch(hash: hash)
            default: nil
            }
            await MainActor.run {
                actionInProgress = nil
                if let result, !result.success {
                    actionError = result.error ?? "Unknown error"
                }
                loadPatches()
                loadStatus()
            }
        }
    }

    private func loadPatches() {
        guard let category = selectedCategory else {
            patches = []
            return
        }
        let filter: String? = switch category {
        case .total: nil as String?
        case .applied: "applied"
        case .pending: "pending"
        case .skipped: "skipped"
        }
        let commits = CauldronBridge.shared.getProtonCommits(filter: filter, limit: 50)
        patches = commits.map { commit in
            let firstLine = String(commit.message.split(separator: "\n").first ?? Substring(commit.message))
            return PatchEntry(
                hash: commit.hash,
                title: firstLine,
                author: commit.author,
                classification: commit.classification,
                transferability: commit.transferability,
                filesChanged: 0,
                status: commit.applied ? "applied" : "pending"
            )
        }
    }
}

// MARK: - Patch Category

enum PatchCategory: Equatable {
    case total, applied, pending, skipped

    var title: String {
        switch self {
        case .total: return "All Patches"
        case .applied: return "Applied Patches"
        case .pending: return "Pending Review"
        case .skipped: return "Skipped Patches"
        }
    }

    var description: String {
        switch self {
        case .total: return "Every patch discovered from Valve's Proton repository."
        case .applied: return "Patches applied to your local Wine source. These improve game compatibility and can be reversed by skipping them."
        case .pending: return "Patches awaiting your review. These may need macOS-specific adaptation before applying."
        case .skipped: return "Patches you chose to skip (Linux-only, build system, etc.). You can apply them later if needed."
        }
    }

    var icon: String {
        switch self {
        case .total: return "number"
        case .applied: return "checkmark.circle.fill"
        case .pending: return "clock.fill"
        case .skipped: return "arrow.uturn.right"
        }
    }

    var tint: Color {
        switch self {
        case .total: return .accentColor
        case .applied: return .green
        case .pending: return .orange
        case .skipped: return .gray
        }
    }
}

// MARK: - Patch Entry Model

struct PatchEntry: Identifiable {
    let id = UUID()
    let hash: String
    let title: String
    let author: String
    let classification: String
    let transferability: String
    let filesChanged: Int
    let status: String // "applied", "pending", "skipped", "conflicted"
}

// MARK: - Patch Row View

private struct PatchRowView: View {
    let patch: PatchEntry
    var analysis: PatchAnalysis? = nil
    let onApply: () -> Void
    let onSkip: () -> Void
    let onReverse: () -> Void
    var onInspect: (() -> Void)? = nil
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Image(systemName: statusIcon)
                    .foregroundStyle(statusColor)
                    .font(.body)

                VStack(alignment: .leading, spacing: 2) {
                    Text(patch.title)
                        .font(.subheadline.weight(.medium))
                        .lineLimit(isExpanded ? nil : 1)

                    HStack(spacing: 8) {
                        Text(String(patch.hash.prefix(8)))
                            .font(.caption2.monospaced())
                            .foregroundStyle(.secondary)

                        Text(patch.classification)
                            .font(.caption2)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 1)
                            .glassEffect(.regular.tint(classificationColor), in: .capsule)
                            .help(classificationHelp)

                        if showTransferability {
                            Text(transferabilityLabel)
                                .font(.caption2)
                                .foregroundStyle(transferabilityColor)
                                .help(transferabilityHelp)
                        }

                        // Analysis badges (after analyze is run)
                        if let a = analysis {
                            if let clean = a.appliesCleanly {
                                Image(systemName: clean ? "checkmark.diamond.fill" : "xmark.diamond.fill")
                                    .font(.caption2)
                                    .foregroundStyle(clean ? .green : .red)
                                    .help(clean ? "Applies cleanly (dry-run passed)" : "Conflicts detected: \(a.conflictFiles.joined(separator: ", "))")
                            }

                            Text(a.impact.localizedCapitalized)
                                .font(.system(size: 9, weight: .semibold))
                                .padding(.horizontal, 5)
                                .padding(.vertical, 1)
                                .background(impactColor(a.impact).opacity(0.2))
                                .foregroundStyle(impactColor(a.impact))
                                .clipShape(Capsule())
                                .help(a.impactReason)

                            if a.canAutoAdapt == true {
                                Text("Auto-adapt")
                                    .font(.system(size: 9, weight: .semibold))
                                    .padding(.horizontal, 5)
                                    .padding(.vertical, 1)
                                    .background(Color.cyan.opacity(0.2))
                                    .foregroundStyle(.cyan)
                                    .clipShape(Capsule())
                                    .help("This patch has Linux-specific code that can be automatically adapted for macOS (\(a.adaptationTransformCount ?? 0) transforms)")
                            }

                            if let rating = a.protondbRating {
                                Text(rating.capitalized)
                                    .font(.system(size: 9, weight: .semibold))
                                    .padding(.horizontal, 5)
                                    .padding(.vertical, 1)
                                    .background(protondbColor(rating).opacity(0.2))
                                    .foregroundStyle(protondbColor(rating))
                                    .clipShape(Capsule())
                                    .help("ProtonDB community rating")
                            }
                        }
                    }
                }

                Spacer()

                if patch.status == "pending" {
                    Button(action: onApply) {
                        Text("Apply")
                            .font(.caption)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 4)
                    }
                    .buttonStyle(.plain)
                    .glassEffect(.regular.tint(.green).interactive(), in: .capsule)

                    Button(action: onSkip) {
                        Text("Skip")
                            .font(.caption)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 4)
                    }
                    .buttonStyle(.plain)
                    .glassEffect(.regular.interactive(), in: .capsule)
                } else if patch.status == "applied" {
                    Button(action: onReverse) {
                        Text("Reverse")
                            .font(.caption)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 4)
                    }
                    .buttonStyle(.plain)
                    .glassEffect(.regular.interactive(), in: .capsule)
                }

                Button {
                    if let onInspect {
                        onInspect()
                    } else {
                        withAnimation(.snappy) { isExpanded.toggle() }
                    }
                } label: {
                    Image(systemName: "info.circle")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .help("Inspect patch details")
            }

            if isExpanded {
                VStack(alignment: .leading, spacing: 6) {
                    PatchDetailRow(label: "Author", value: patch.author)
                    PatchDetailRow(label: "Classification", value: patch.classification)
                    PatchDetailRow(label: "Transferability", value: transferabilityLabel)
                    PatchDetailRow(label: "Full Hash", value: patch.hash)

                    if let a = analysis {
                        Divider().padding(.vertical, 2)

                        PatchDetailRow(label: "Lines", value: "+\(a.linesAdded) / -\(a.linesRemoved)")
                        PatchDetailRow(label: "Impact", value: "\(a.impact.localizedCapitalized) — \(a.impactReason)")

                        if let clean = a.appliesCleanly {
                            PatchDetailRow(label: "Dry Run", value: clean ? "Applies cleanly" : "Conflicts: \(a.conflictFiles.joined(separator: ", "))")
                        }

                        if !a.affectedDlls.isEmpty {
                            PatchDetailRow(label: "DLLs", value: a.affectedDlls.joined(separator: ", "))
                        }

                        if !a.affectedGames.isEmpty {
                            PatchDetailRow(label: "Affects", value: a.affectedGames.joined(separator: ", "))
                        }

                        if let rating = a.protondbRating {
                            PatchDetailRow(label: "ProtonDB", value: rating.capitalized)
                        }

                        if a.canAutoAdapt == true {
                            PatchDetailRow(label: "Adaptation", value: "\(a.adaptationTransformCount ?? 0) auto-transforms (\(a.adaptationConfidence ?? "unknown") confidence)")
                        }

                        if let warnings = a.adaptationWarnings, !warnings.isEmpty {
                            PatchDetailRow(label: "Warnings", value: warnings.joined(separator: "; "))
                        }
                    }
                }
                .padding(.leading, 28)
                .padding(.top, 4)
                .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
        .padding(.vertical, 6)
    }

    private var statusIcon: String {
        PatchDisplayHelpers.statusIcon(patch.status)
    }

    private var statusColor: Color {
        PatchDisplayHelpers.statusColor(patch.status)
    }

    private var classificationColor: Color {
        PatchDisplayHelpers.classificationColor(patch.classification)
    }

    private var classificationHelp: String {
        switch patch.classification {
        case "WineApiFix": return "Wine API Fix — Changes to Wine's core DLLs, server, or loader. These implement Windows API calls and are usually portable to macOS."
        case "DxvkFix": return "DXVK Fix — Changes to DXVK, which translates DirectX 9/10/11 to Vulkan. Works with MoltenVK on macOS."
        case "Vkd3dFix": return "VKD3D-Proton Fix — Changes to VKD3D-Proton, which translates DirectX 12 to Vulkan. May need Metal/MoltenVK adjustments."
        case "GameConfig": return "Game Config — Game-specific settings, app IDs, or compatibility tweaks. Usually directly applicable."
        case "KernelWorkaround": return "Kernel Workaround — Uses Linux-specific kernel features (futex, eventfd, epoll). Needs translation to macOS equivalents (MSync, kqueue)."
        case "SteamIntegration": return "Steam Integration — Changes to lsteamclient or vrclient. May need macOS-specific path adjustments."
        case "BuildSystem": return "Build System — Makefiles, CI configs, submodule updates. Not applicable to your Wine source."
        default: return "Unclassified — Could not determine the type of change. Manual review recommended."
        }
    }

    private var showTransferability: Bool {
        patch.transferability != "None"
    }

    private var transferabilityLabel: String {
        PatchDisplayHelpers.transferabilityLabel(patch.transferability)
    }

    private var transferabilityColor: Color {
        PatchDisplayHelpers.transferabilityColor(patch.transferability)
    }

    private func impactColor(_ impact: String) -> Color {
        PatchDisplayHelpers.impactColor(impact)
    }

    private func protondbColor(_ rating: String) -> Color {
        PatchDisplayHelpers.protondbColor(rating)
    }

    private var transferabilityHelp: String {
        switch patch.transferability {
        case "High": return "Portable — This patch can likely be applied directly to macOS Wine without modification."
        case "Medium": return "Needs review — May work on macOS but should be reviewed for platform-specific assumptions."
        case "Low": return "Needs adaptation — Uses Linux-specific features that need macOS equivalents (e.g. futex → MSync)."
        case "None": return "Not applicable — Build system or infrastructure change that doesn't affect Wine functionality."
        default: return "Unknown transferability level."
        }
    }
}

// MARK: - Detail Row

private struct PatchDetailRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack(spacing: 8) {
            Text(label + ":")
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(width: 100, alignment: .trailing)
            Text(value)
                .font(.caption.monospaced())
                .foregroundStyle(.primary)
                .textSelection(.enabled)
        }
    }
}

// MARK: - Clickable Stat Card

private struct PatchStatCard: View {
    let label: String
    let value: String
    let icon: String
    let tint: Color
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: 6) {
                Image(systemName: icon)
                    .font(.title3)
                    .foregroundStyle(tint)
                Text(value)
                    .font(.title2.weight(.bold).monospacedDigit())
                Text(label)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 14)
            .padding(.horizontal, 8)
        }
        .buttonStyle(.plain)
        .glassEffect(
            isSelected
                ? .regular.tint(tint).interactive()
                : .regular.interactive(),
            in: .rect(cornerRadius: 12)
        )
    }
}

// MARK: - Nested Patch Group (collapsible)

private struct NestedPatchGroup: View {
    let entry: SyncStatusView.NestedEntry
    var analyses: [String: PatchAnalysis] = [:]
    var onInspect: ((PatchEntry) -> Void)? = nil
    let onAction: (String, String) -> Void
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Group header
            Button {
                withAnimation(.snappy) { isExpanded.toggle() }
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "chevron.right")
                        .rotationEffect(.degrees(isExpanded ? 90 : 0))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .frame(width: 16)

                    Image(systemName: "clock.fill")
                        .foregroundStyle(.orange)
                        .font(.body)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(entry.label)
                            .font(.subheadline.weight(.medium))
                            .foregroundStyle(.primary)

                        HStack(spacing: 6) {
                            Text(entry.patch.classification)
                                .font(.caption2)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 1)
                                .background(Color.secondary.opacity(0.15))
                                .clipShape(Capsule())

                            Text(entry.patch.transferability)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }

                    Spacer()

                    Text("\(entry.children.count + 1)")
                        .font(.caption.weight(.medium))
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 2)
                        .background(Color.secondary.opacity(0.15))
                        .clipShape(Capsule())
                }
                .padding(.vertical, 6)
            }
            .buttonStyle(.plain)

            if isExpanded {
                VStack(spacing: 0) {
                    let all = [entry.patch] + entry.children
                    ForEach(all, id: \.hash) { patch in
                        PatchRowView(patch: patch, analysis: analyses[patch.hash], onApply: {
                            onAction(patch.hash, "apply")
                        }, onSkip: {
                            onAction(patch.hash, "skip")
                        }, onReverse: {
                            onAction(patch.hash, "reverse")
                        }, onInspect: {
                            onInspect?(patch)
                        })
                    }
                }
                .padding(.leading, 24)
                .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
    }
}

