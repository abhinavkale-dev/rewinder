import SwiftUI

@main
struct RewinderMainApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var engine = RewinderEngine()

    var body: some Scene {
        WindowGroup {
            ContentView(engine: engine)
                .task { appDelegate.attach(engine) }
        }
        .windowResizability(.contentSize)
    }
}
