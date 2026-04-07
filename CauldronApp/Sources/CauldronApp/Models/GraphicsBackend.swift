import SwiftUI

enum GraphicsBackend: String, CaseIterable, Codable {
    case d3dMetal = "d3d_metal"
    case dxmt = "dxmt"
    case dxvkMoltenVK = "dxvk_moltenvk"
    case dxvkKosmicKrisp = "dxvk_kosmic_krisp"
    case vkd3dProton = "vkd3d_proton"
    case auto = "auto"

    var displayName: String {
        switch self {
        case .d3dMetal: return "D3DMetal"
        case .dxmt: return "DXMT"
        case .dxvkMoltenVK: return "DXVK + MoltenVK"
        case .dxvkKosmicKrisp: return "DXVK + Kosmic Krisp"
        case .vkd3dProton: return "VKD3D-Proton"
        case .auto: return "Auto"
        }
    }

    var tintColor: Color {
        switch self {
        case .d3dMetal: return .blue
        case .dxmt: return .purple
        case .dxvkMoltenVK: return .orange
        case .dxvkKosmicKrisp: return .mint
        case .vkd3dProton: return .red
        case .auto: return .secondary
        }
    }
}
