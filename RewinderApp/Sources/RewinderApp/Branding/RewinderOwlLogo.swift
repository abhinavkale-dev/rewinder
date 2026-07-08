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
        .scaleEffect(scale, anchor: .center)
        .frame(width: Self.viewBox.width * scale, height: height)
        .accessibilityElement()
        .accessibilityLabel("Rewinder")
    }

    private var lidScale: CGFloat { max(eyeOpenness, 0.06) }

    private var catchlightOpacity: Double { eyeOpenness < 0.3 ? 0 : 1 }

    private func eyeAnchor(cx: CGFloat, cy: CGFloat) -> UnitPoint {
        UnitPoint(x: cx / Self.viewBox.width, y: cy / Self.viewBox.height)
    }

    private func eyeOutline(cx: CGFloat, cy: CGFloat, rx: CGFloat, ry: CGFloat) -> some View {
        Ellipse()
            .stroke(.black, lineWidth: 3)
            .frame(width: rx * 2, height: ry * 2)
            .position(x: cx, y: cy)
    }

    private func ellipse(cx: CGFloat, cy: CGFloat, rx: CGFloat, ry: CGFloat, fill: Color) -> some View {
        Ellipse()
            .fill(fill)
            .frame(width: rx * 2, height: ry * 2)
            .position(x: cx, y: cy)
    }
}

private struct OwlPath: Shape {
    let d: String

    init(_ d: String) { self.d = d }

    func path(in rect: CGRect) -> Path {
        let raw = SVGPathParser.path(from: d)
        let sx = rect.width / 720
        let sy = rect.height / 662
        let transform = CGAffineTransform(a: sx, b: 0, c: 0, d: sy, tx: rect.minX, ty: rect.minY)
        return raw.applying(transform)
    }
}

extension RewinderOwlLogo {
    fileprivate static let backgroundD = "M629.397 0.343563C639.794 -1.13812 651.807 2.05774 660.702 12.1756C677.34 31.1018 684.257 59.4075 676.239 94.0795C686.171 97.3554 695.331 105.535 698.936 118.428L699.326 119.94L699.637 121.322C702.658 135.637 699.681 150.7 693.455 164.87C688.375 176.432 680.528 188.841 669.522 202.244C679.799 224.586 691.383 257.917 690.59 290.719C690.171 308.034 693.424 316.396 695.704 321.016C697.282 324.215 698.493 325.949 701.164 330.413C702.944 333.388 707.701 341.136 709.176 351.295C709.119 350.93 709.295 351.655 710.52 355.388C711.728 359.07 713.751 365.256 714.944 372.499C717.418 387.506 716.032 405.133 705.504 423.34C703.999 437.804 697.994 456.544 683.823 474.904C667.036 496.652 581.243 641.173 374.631 653.264C306.948 658.319 151.423 634.421 56.9646 496.194C43.0499 485.119 22.5494 461.686 18.6607 427.873C8.24132 416.21 -0.0541663 397.262 6.11087 373.1C9.58923 359.468 16.5551 342.655 20.9181 331.466C23.3797 325.153 25.1858 320.279 26.2941 316.626C26.4638 316.067 26.5998 315.586 26.7101 315.179C26.3497 308.644 27.0077 301.349 27.6788 295.651C28.6879 287.085 30.4425 276.656 33.0461 265.369C37.175 247.47 43.7791 225.941 53.846 205.76C47.1961 197.372 40.308 187.648 34.7624 177.622C30.5351 169.98 26.3311 160.829 24.0425 151.043C21.8097 141.495 20.5905 127.711 26.9865 114.088C29.5089 108.715 34.4852 99.9748 43.5682 94.4203C40.5468 75.2932 41.749 53.7743 51.1901 31.5703L51.6962 30.4208C62.4332 6.96299 88.2727 3.12253 105.302 13.7526L106.119 14.2761L111.571 17.8546C124.772 26.4301 141.15 36.4664 156.671 44.8182C165.553 49.5978 173.758 53.6132 180.688 56.4643C183.12 57.4648 185.273 58.2656 187.14 58.8933C221.736 37.1882 291.806 5.41546 366.619 10.831C387.588 11.4001 419.811 14.3274 451.655 21.3493C479.068 27.3943 510.963 37.4395 534.095 55.1414C555.142 50.1504 586.033 37.4453 605.505 14.0252L606.622 12.7394C612.351 6.40308 620.25 1.64721 629.397 0.343563ZM27.0563 318.99L27.1668 319.799C27.123 319.512 27.0887 319.221 27.0505 318.928C27.0529 318.948 27.0538 318.969 27.0563 318.99Z"

    fileprivate static let bodyD = "M81.0239 34.678C59.7921 75.7424 80.1473 116.396 96.1436 135.615C97.7155 137.503 96.5929 140.306 94.1845 139.819C75.127 135.966 63.6893 102.605 53.6062 124.082C45.0449 142.318 72.9149 178.493 89.4334 195.993C90.4233 197.042 90.5469 198.632 89.7407 199.828C63.0482 239.417 54.9902 300.327 56.5462 310.507C58.1293 320.864 40.4884 355.066 34.8345 377.225C30.7387 393.277 39.6859 402.904 46.0589 406.607C47.1604 407.247 47.8816 408.43 47.8289 409.702C46.5802 439.896 66.7124 461.296 77.6357 468.704C77.9732 468.932 78.2592 469.221 78.4847 469.561C164.54 599.242 310.512 622.168 372.816 617.403C564.897 606.232 643.102 473.475 660.787 450.562C674.097 433.319 676.51 417.129 676.041 410.043C675.989 409.252 676.202 408.447 676.668 407.806C695.24 382.298 681.188 363.423 679.749 352.115C678.29 340.659 659.693 330.95 660.787 285.707C661.628 250.939 643.989 212.075 634.012 195.066C633.32 193.886 633.513 192.374 634.456 191.382C673.392 150.486 674.052 127.339 668.498 118.938C667.672 117.689 665.945 117.71 664.798 118.673C638.262 140.957 628.165 138.132 625.94 133.066C625.63 132.359 625.814 131.536 626.218 130.877C663.295 70.4049 648.787 36.9389 635.108 25.9858C633.872 24.9956 632.103 25.3631 631.147 26.6261C601.312 66.0291 550.465 81.4205 526.958 84.4617C526.008 84.5846 525.094 84.2316 524.428 83.5422C491.823 49.7835 404.981 39.3278 365.235 38.3317C291.42 32.745 220.875 68.4316 194.041 87.4715C193.552 87.8182 193.023 88.0047 192.424 87.9952C170.431 87.6441 113.72 52.8558 85.2908 33.6275C83.834 32.6422 81.8317 33.1158 81.0239 34.678Z"

    fileprivate static let faceD = "M113.762 441.029C58.9656 355.949 96.9382 260.451 122.774 223.338C224.75 101.955 330.854 198.259 359.536 267.418C359.864 268.209 360.985 268.186 361.387 267.43C437.115 124.805 537.296 164.9 578.086 202.99C658.549 292.063 636.607 390.863 615.578 429.128C542.403 547.916 418.478 526.174 411.756 525.511C411.554 525.491 411.442 525.56 411.275 525.674C367.467 555.662 318.687 538.341 299.387 525.713C299.17 525.57 298.929 525.521 298.673 525.567C259.523 532.746 168.377 525.83 113.762 441.029Z"

    fileprivate static let beakD = "M329.206 402.98C325.853 377.987 342.954 361.762 359.446 361.766C374.632 361.769 394.912 372.692 391.732 402.98C388.966 429.325 367.756 446.253 359.11 452.137C358.764 452.373 358.322 452.363 357.988 452.111C349.409 445.625 332.486 427.436 329.206 402.98Z"

    fileprivate static let leftPupilD = "M241.802 298.883C241.802 311.408 232.268 321.562 220.507 321.562C208.747 321.562 199.213 311.408 199.213 298.883C199.213 286.358 208.747 276.204 220.507 276.204C232.268 276.204 241.802 286.358 241.802 298.883Z"
}

#Preview {
    RewinderOwlLogo(height: 240)
        .padding(60)
        .background(Color(.sRGB, red: 0.110, green: 0.114, blue: 0.129, opacity: 1))
}
