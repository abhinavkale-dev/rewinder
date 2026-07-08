import SwiftUI

enum StatusPopover {
    case screen, mic
}

struct DisplayDevice: Identifiable {
    let id: String
    let name: String
    let isMain: Bool
}

extension DisplayDevice {
    static func connected() -> [DisplayDevice] {
        NSScreen.screens.compactMap { screen in
            guard let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber
            else { return nil }
            let id = CGDirectDisplayID(truncating: number)
            return DisplayDevice(id: String(id), name: screen.localizedName, isMain: id == CGMainDisplayID())
        }
    }

    func isEffectiveSelection(storedId: String?, in all: [DisplayDevice]) -> Bool {
        if let storedId, !storedId.isEmpty, all.contains(where: { $0.id == storedId }) {
            return id == storedId
        }
        return isMain
    }
}

struct FillAnchor: Equatable {
    let start: Date
    let secs: Double

    func fill(at date: Date) -> Double {
        secs + max(0, date.timeIntervalSince(start))
    }
}

enum HomePhase { case off, permission, starting, building, protected, saving }
