import SwiftUI
import AppKit

class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)

        // Set app icon from resource bundle
        if let url = Bundle.module.url(forResource: "AppIcon", withExtension: "png"),
           let image = NSImage(contentsOf: url) {
            NSApp.applicationIconImage = image
        }

        // Sync Metal HUD global defaults with saved preference on startup
        let hudEnabled = UserDefaults.standard.bool(forKey: "metalPerformanceHUD")
        let globalDefaults = UserDefaults(suiteName: UserDefaults.globalDomain)
        globalDefaults?.set(hudEnabled, forKey: "MetalForceHudEnabled")
    }
}

@main
struct CauldronApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @State private var licenseManager = LicenseManager()
    private let bridge: CauldronBridge

    init() {
        bridge = CauldronBridge.shared
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(BottleListViewModel(bridge: bridge))
                .environment(licenseManager)
                .sheet(isPresented: showActivationSheet) {
                    ActivationView()
                        .environment(licenseManager)
                        .interactiveDismissDisabled()
                }
                // Runtimes (DXVK, DXMT, MoltenVK) are bundled with the app.
                // No first-boot download needed. Users can download newer
                // versions manually via Settings > Advanced.
        }
        .windowStyle(.automatic)
        .defaultSize(width: 960, height: 640)

        Settings {
            SettingsView()
                .environment(licenseManager)
        }
    }

    private var showActivationSheet: Binding<Bool> {
        Binding(
            get: { licenseManager.status == .expired && BuildChannel.requiresActivation },
            set: { _ in }
        )
    }
}
