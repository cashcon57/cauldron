import Foundation
import SwiftUI

@MainActor
@Observable
final class BottleListViewModel {
    var bottles: [Bottle] = []
    var selectedBottle: Bottle?
    var isCreatingBottle: Bool = false

    private let bridge: CauldronBridge

    init(bridge: CauldronBridge) {
        self.bridge = bridge
        loadBottles()
    }

    func loadBottles() {
        #if DEBUG
        if ProcessInfo.processInfo.environment["CAULDRON_PREVIEW"] == "1" {
            bottles = Self.mockBottles
            return
        }
        #endif
        bottles = bridge.listBottles()
    }

    func createBottle(name: String, wineVersion: String) {
        #if DEBUG
        if ProcessInfo.processInfo.environment["CAULDRON_PREVIEW"] == "1" {
            let mock = Bottle(
                id: UUID().uuidString,
                name: name,
                path: "/mock/path/\(name)",
                wineVersion: wineVersion,
                graphicsBackend: GraphicsBackend.auto.rawValue,
                createdAt: ISO8601DateFormatter().string(from: .now)
            )
            bottles.append(mock)
            selectedBottle = mock
            return
        }
        #endif
        if let bottle = bridge.createBottle(name: name, wineVersion: wineVersion) {
            bottles.append(bottle)
            selectedBottle = bottle
        }
    }

    func deleteBottle(_ bottle: Bottle) {
        #if DEBUG
        if ProcessInfo.processInfo.environment["CAULDRON_PREVIEW"] == "1" {
            bottles.removeAll { $0.id == bottle.id }
            if selectedBottle?.id == bottle.id {
                selectedBottle = nil
            }
            return
        }
        #endif
        if bridge.deleteBottle(id: bottle.id) {
            bottles.removeAll { $0.id == bottle.id }
            if selectedBottle?.id == bottle.id {
                selectedBottle = nil
            }
        }
    }

    #if DEBUG
    static let mockBottles: [Bottle] = [
        Bottle(
            id: "1",
            name: "Default",
            path: "/Users/mock/Library/Application Support/Cauldron/bottles/Default",
            wineVersion: "wine-10.0",
            graphicsBackend: GraphicsBackend.d3dMetal.rawValue,
            createdAt: "2026-03-15T10:30:00Z"
        ),
        Bottle(
            id: "2",
            name: "Gaming",
            path: "/Users/mock/Library/Application Support/Cauldron/bottles/Gaming",
            wineVersion: "wine-10.0",
            graphicsBackend: GraphicsBackend.dxvkMoltenVK.rawValue,
            createdAt: "2026-03-20T14:00:00Z"
        ),
        Bottle(
            id: "3",
            name: "Legacy",
            path: "/Users/mock/Library/Application Support/Cauldron/bottles/Legacy",
            wineVersion: "wine-9.0",
            graphicsBackend: GraphicsBackend.auto.rawValue,
            createdAt: "2026-02-10T08:00:00Z"
        ),
    ]
    #endif
}
