import SwiftUI
import AppKit

class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }
}

@main
struct CauldronApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    private let bridge: CauldronBridge

    init() {
        bridge = CauldronBridge.shared
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(BottleListViewModel(bridge: bridge))
        }
        .windowStyle(.automatic)
        .defaultSize(width: 960, height: 640)

        Settings {
            SettingsView()
        }
    }
}
