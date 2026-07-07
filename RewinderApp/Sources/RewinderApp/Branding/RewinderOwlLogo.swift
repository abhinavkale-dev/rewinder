import SwiftUI

struct RewinderOwlLogo: View {
    var height: CGFloat = 120
    var eyeOpenness: CGFloat = 1
    var pupilShift: CGFloat = 0

    private static let viewBox = CGSize(width: 720, height: 662)
    private var scale: CGFloat { height / Self.viewBox.height }

    private let bodyBlue = Color(.sRGB, red: 0.145, green: 0.384, blue: 0.984, opacity: 1)
    private let eyeDark = Color(.sRGB, red: 0.059, green: 0.047, blue: 0.027, opacity: 1)
    private let bgEnd = Color(.sRGB, red: 0.910, green: 0.922, blue: 1.0, opacity: 1)

    private var bgGradient: RadialGradient {
        RadialGradient(
            gradient: Gradient(stops: [
                .init(color: .white, location: 0.27),
                .init(color: bgEnd, location: 1),
            ]),
            center: .center, startRadius: 0, endRadius: 380
        )
    }

    var body: some View {
        ZStack {
            OwlPath(Self.backgroundD)
                .fill(bgGradient)
                .shadow(color: .black.opacity(0.28), radius: 9, x: 0, y: 6)

            OwlPath(Self.bodyD).fill(bodyBlue)
            OwlPath(Self.bodyD).stroke(.black, lineWidth: 4)

            OwlPath(Self.faceD).fill(.white)

            OwlPath(Self.beakD).stroke(.black, lineWidth: 4)

            ZStack {
                eyeOutline(cx: 231.722, cy: 327.053, rx: 81.5504, ry: 92.0288)
                Group {
                    ellipse(cx: 246.0795, cy: 315.6884, rx: 64.7795, ry: 70.7504, fill: eyeDark)
                    OwlPath(Self.leftPupilD).fill(.white)
                        .opacity(catchlightOpacity)
                }
                .offset(x: pupilShift)
            }
            .scaleEffect(x: 1, y: lidScale, anchor: eyeAnchor(cx: 231.722, cy: 327.053))

            ZStack {
                eyeOutline(cx: 489.1816, cy: 327.0528, rx: 81.5504, ry: 92.0288)
                Group {
                    ellipse(cx: 474.8255, cy: 315.6884, rx: 64.7795, ry: 70.7504, fill: eyeDark)
                    ellipse(cx: 448.7265, cy: 298.8828, rx: 21.2945, ry: 22.6788, fill: .white)
                        .opacity(catchlightOpacity)
                }
                .offset(x: pupilShift)
            }
            .scaleEffect(x: 1, y: lidScale, anchor: eyeAnchor(cx: 489.1816, cy: 327.0528))
        }
        .frame(width: Self.viewBox.width, height: Self.viewBox.height)
