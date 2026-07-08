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
            case .checking:
                if let i = next.firstIndex(where: { $0.id == item.id }) { next[i].stage = .granted }
                scheduleDrop(item.id)
            case .idle:
                next.removeAll { $0.id == item.id }
            case .granted:
                break
            }
        }
        for alert in alerts where !items.contains(where: { $0.id == alert.id }) {
            next.append(ChipItem(alert: alert, stage: .idle))
        }
        for alert in alerts {
            if let i = next.firstIndex(where: { $0.id == alert.id }), next[i].stage != .granted {
                next[i].alert = alert
            }
        }

        if animated { mutate { items = next } } else { items = next }
    }

    private func scheduleDrop(_ id: String) {
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(900))
            mutate { items.removeAll { $0.id == id } }
        }
    }

    private func scheduleCheckingTimeout(_ id: String) {
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(32))
            guard let i = items.firstIndex(where: { $0.id == id }), items[i].stage == .checking else { return }
            mutate { items[i].stage = .idle }
        }
    }

    private func mutate(_ change: () -> Void) {
        if let spring { withAnimation(spring, change) } else { change() }
    }

    private func checkingVerb(_ kind: AlertKind) -> String {
        switch kind {
        case .resume: return "Resuming…"
        case .restart: return "Restarting…"
        default: return "Checking…"
        }
    }

    private func doneVerb(_ kind: AlertKind) -> String {
        switch kind {
        case .resume: return "Resumed"
        case .restart: return "Restarted"
        default: return "Granted"
        }
    }
}

struct ChipItem: Identifiable {
    var alert: HomeAlert
    var stage: PermissionChip.Stage
    var id: String { alert.id }
}
