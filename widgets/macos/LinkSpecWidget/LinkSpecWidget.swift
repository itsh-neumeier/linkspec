// LinkSpec WidgetKit extension (ADR 0005).
// Display only: reads the snapshot that the LinkSpec app writes to the
// app group container. No diagnosis logic here.

import SwiftUI
import WidgetKit

struct PortSnapshot: Decodable, Identifiable {
    var id: String { label }
    let label: String
    let headline: String
    let headline_text: String
    let summary: String
    let status: String
}

struct Snapshot: Decodable {
    let schema: String
    let generated_at: String
    let ports: [PortSnapshot]
}

struct Entry: TimelineEntry {
    let date: Date
    let snapshot: Snapshot?
}

private let appGroup = "group.io.github.itsh-neumeier.linkspec"

func loadSnapshot() -> Snapshot? {
    guard let dir = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroup),
          let data = try? Data(contentsOf: dir.appendingPathComponent("widget.json"))
    else { return nil }
    return try? JSONDecoder().decode(Snapshot.self, from: data)
}

struct Provider: TimelineProvider {
    func placeholder(in context: Context) -> Entry { Entry(date: .now, snapshot: nil) }
    func getSnapshot(in context: Context, completion: @escaping (Entry) -> Void) {
        completion(Entry(date: .now, snapshot: loadSnapshot()))
    }
    func getTimeline(in context: Context, completion: @escaping (Timeline<Entry>) -> Void) {
        let entry = Entry(date: .now, snapshot: loadSnapshot())
        completion(Timeline(entries: [entry], policy: .after(.now.addingTimeInterval(15 * 60))))
    }
}

func symbol(_ status: String) -> String {
    switch status {
    case "ok": return "checkmark.circle.fill"
    case "warn": return "exclamationmark.triangle.fill"
    default: return "xmark.octagon.fill"
    }
}

struct LinkSpecWidgetView: View {
    @Environment(\.widgetFamily) var family
    let entry: Entry

    var body: some View {
        let ports = entry.snapshot?.ports ?? []
        let limit = family == .systemSmall ? 2 : (family == .systemMedium ? 3 : 6)
        VStack(alignment: .leading, spacing: 6) {
            Text("LinkSpec").font(.headline)
            if ports.isEmpty {
                Text(String(localized: "Keine Daten – LinkSpec-App öffnen")).font(.caption)
            }
            ForEach(ports.prefix(limit)) { p in
                HStack(alignment: .top, spacing: 6) {
                    Image(systemName: symbol(p.status))
                        .accessibilityLabel(p.status)
                    VStack(alignment: .leading, spacing: 1) {
                        Text("\(p.label): \(p.headline_text)").font(.caption).bold()
                        if family != .systemSmall && !p.summary.isEmpty {
                            Text(p.summary).font(.caption2).lineLimit(2)
                        }
                    }
                }
            }
        }
        .containerBackground(.fill.tertiary, for: .widget)
    }
}

@main
struct LinkSpecWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "LinkSpecWidget", provider: Provider()) { entry in
            LinkSpecWidgetView(entry: entry)
        }
        .configurationDisplayName("LinkSpec")
        .description(String(localized: "USB-C-Ports auf einen Blick"))
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}
