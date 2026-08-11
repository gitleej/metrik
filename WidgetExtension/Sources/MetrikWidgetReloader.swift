import WidgetKit

@main
enum MetrikWidgetReloader {
    static func main() {
        WidgetCenter.shared.reloadAllTimelines()
    }
}
