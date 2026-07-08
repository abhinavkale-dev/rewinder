import SwiftUI

struct PermissionChips: View {
    let alerts: [HomeAlert]
    let onGrant: (HomeAlert) -> Void
    let onRecheck: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @State private var items: [ChipItem] = []

    private var spring: Animation? {
        reduceMotion ? nil : .spring(response: 0.34, dampingFraction: 0.78)
    }

    var body: some View {
        VStack(spacing: 8) {
            ForEach(items) { item in
                PermissionChip(
                    tone: item.alert.tone,
                    icon: item.alert.icon,
                    title: label(for: item),
                    stage: item.stage,
                    accessibilityTitle: item.alert.title,
                    accessibilityHint: item.alert.message
                ) {
                    tap(item)
                }
                .transition(.scale(scale: 0.96, anchor: .top).combined(with: .opacity))
            }
        }
        .onAppear { reconcile(animated: false) }
        .onChange(of: alerts.map(\.id)) { _, _ in reconcile(animated: true) }
    }

    private func label(for item: ChipItem) -> String {
        switch item.stage {
        case .idle: return item.alert.actionTitle
        case .checking: return checkingVerb(item.alert.kind)
        case .granted: return doneVerb(item.alert.kind)
        }
    }

    private func tap(_ item: ChipItem) {
        guard let i = items.firstIndex(where: { $0.id == item.id }) else { return }
        switch items[i].stage {
        case .idle:
            mutate { items[i].stage = .checking }
            onGrant(item.alert)
            scheduleCheckingTimeout(item.id)
        case .checking:
            onRecheck()
        case .granted:
            break
        }
    }

    private func reconcile(animated: Bool) {
        let liveIDs = Set(alerts.map(\.id))
        var next = items

        for item in items where !liveIDs.contains(item.id) {
            switch item.stage {
