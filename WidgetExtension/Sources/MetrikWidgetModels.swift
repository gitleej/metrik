import Foundation

/// Widget clicks open the host app through these URLs; the host maps them to
/// expanded-view navigation targets (see nav_for_widget_url in macos.rs).
enum MetrikWidgetDeepLink {
    static let open = URL(string: "metrik://open")!

    static func agent(_ id: String) -> URL {
        URL(string: "metrik://agent/\(id)")!
    }
}

struct MetrikWidgetSnapshot: Decodable {
    let schemaVersion: Int
    let generatedAt: String
    let totalTokens: Int64
    let agents: [MetrikWidgetAgent]
}

struct MetrikWidgetAgent: Decodable, Identifiable {
    let id: String
    let label: String
    let tokens: Int64
    let windows: [MetrikWidgetQuotaWindow]

    var bindingWindow: MetrikWidgetQuotaWindow? {
        let live = self.windows.filter { $0.available && !$0.resetExpired }
        guard let shortest = live.first else { return nil }
        let low = live.filter { $0.remainingPercent <= 15 }
        return low.min { $0.remainingPercent < $1.remainingPercent } ?? shortest
    }

    var asset: (name: String, ext: String)? {
        switch self.id {
        case "codex": ("chatgpt-app-icon", "png")
        case "claude": ("claude-app-icon", "jpg")
        case "zcode": ("zcode-app-icon", "png")
        case "opencode": ("opencode-app-icon", "png")
        case "kimi": ("kimi-app-icon", "png")
        case "antigravity": ("antigravity-app-icon", "png")
        default: nil
        }
    }
}

struct MetrikWidgetQuotaWindow: Decodable {
    let key: String
    let label: String
    let available: Bool
    let remainingPercent: Double
    let resetsInMinutes: Double?
    let stale: Bool
    let resetExpired: Bool
    let quality: String

    var roundedRemaining: Int {
        Int(self.remainingPercent.rounded().clamped(to: 0 ... 100))
    }
}

private extension Comparable {
    func clamped(to limits: ClosedRange<Self>) -> Self {
        min(max(self, limits.lowerBound), limits.upperBound)
    }
}

enum MetrikWidgetStore {
    static let fileName = "widget-snapshot.json"

    static func load() -> MetrikWidgetSnapshot? {
        if let snapshotURL = self.snapshotURL, let snapshot = self.decode(snapshotURL) {
            return snapshot
        }

        guard let bundledURL = Bundle.main.url(
            forResource: "preview-widget-snapshot",
            withExtension: "json")
        else { return nil }
        return self.decode(bundledURL)
    }

    // The host publishes the snapshot into Application Support. Release builds are
    // ad-hoc signed without a team identity, so an App Group container cannot
    // replace this plain per-user file bridge.
    private static var snapshotURL: URL? {
        FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first?
            .appendingPathComponent("Metrik/Widget", isDirectory: true)
            .appendingPathComponent(self.fileName, isDirectory: false)
    }

    private static func decode(_ url: URL) -> MetrikWidgetSnapshot? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(MetrikWidgetSnapshot.self, from: data)
    }

    static let preview = MetrikWidgetSnapshot(
        schemaVersion: 1,
        generatedAt: ISO8601DateFormatter().string(from: Date()),
        totalTokens: 43_300_000,
        agents: [
            MetrikWidgetAgent(
                id: "codex",
                label: "ChatGPT",
                tokens: 30_400_000,
                windows: [
                    MetrikWidgetQuotaWindow(
                        key: "seven_day",
                        label: "每周",
                        available: true,
                        remainingPercent: 89,
                        resetsInMinutes: 7_620,
                        stale: false,
                        resetExpired: false,
                        quality: "official_live")
                ]),
            MetrikWidgetAgent(
                id: "claude",
                label: "Claude",
                tokens: 9_700_000,
                windows: []),
            MetrikWidgetAgent(
                id: "zcode",
                label: "GLM",
                tokens: 2_300_000,
                windows: []),
            MetrikWidgetAgent(
                id: "opencode",
                label: "OpenCode",
                tokens: 900_000,
                windows: []),
            MetrikWidgetAgent(
                id: "kimi",
                label: "Kimi",
                tokens: 620_000,
                windows: []),
            MetrikWidgetAgent(
                id: "antigravity",
                label: "Antigravity",
                tokens: 410_000,
                windows: [])
        ])
}

enum MetrikWidgetFormat {
    static func tokens(_ value: Int64) -> String {
        let number = Double(max(value, 0))
        if number >= 1_000_000 {
            return String(format: number >= 10_000_000 ? "%.1fM" : "%.2fM", number / 1_000_000)
        }
        if number >= 1_000 {
            return String(format: number >= 100_000 ? "%.0fK" : "%.1fK", number / 1_000)
        }
        return String(value)
    }

    static func reset(_ minutes: Double?) -> String? {
        guard let minutes, minutes.isFinite, minutes > 0 else { return nil }
        let totalHours = Int(ceil(minutes / 60))
        let days = totalHours / 24
        let hours = totalHours % 24
        if days > 0, hours > 0 { return "\(days) 天 \(hours) 小时后重置" }
        if days > 0 { return "\(days) 天后重置" }
        return "\(max(hours, 1)) 小时后重置"
    }
}
