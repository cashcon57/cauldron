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

        // Metal HUD is now scoped per-launch via MTL_HUD_ENABLED env var in the
        // Rust launch pipeline (cauldron-bridge). No global defaults writes —
        // those leaked the HUD into every Metal app on the system (Photoshop, etc.).
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
