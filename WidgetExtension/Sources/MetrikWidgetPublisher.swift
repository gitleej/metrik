import Foundation

@main
enum MetrikWidgetPublisher {
    static func main() {
        guard let container = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: "group.app.metrik.desktop")
        else {
            FileHandle.standardError.write(Data("App Group container is unavailable\n".utf8))
            exit(2)
        }

        let data = FileHandle.standardInput.readDataToEndOfFile()
        guard !data.isEmpty else {
            FileHandle.standardError.write(Data("Widget snapshot input is empty\n".utf8))
            exit(3)
        }

        let target = container.appendingPathComponent("widget-snapshot.json", isDirectory: false)
        do {
            try data.write(to: target, options: [.atomic])
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o600],
                ofItemAtPath: target.path)
            FileHandle.standardOutput.write(Data("\(target.path)\n".utf8))
        } catch {
            FileHandle.standardError.write(Data("\(error)\n".utf8))
            exit(4)
        }
    }
}
