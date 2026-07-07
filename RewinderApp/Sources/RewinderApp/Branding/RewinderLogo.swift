import SwiftUI

struct RewinderRShape: Shape {
    private static let subpaths: [String] = [
        "M8 327.63V143.549C8 141.893 9.40104 140.546 11.0575 140.583C115.397 142.91 127.543 264.502 120.323 328.042C120.152 329.545 118.879 330.63 117.366 330.63H11C9.34315 330.63 8 329.287 8 327.63Z",
        "M8 120.686V10C8 8.34315 9.34314 7 11 7H175.226C177.277 7 178.705 9.06361 178.019 10.9963C139.654 119.002 50.5484 130.192 10.4093 123.594C8.9891 123.361 8 122.126 8 120.686Z",
        "M123.517 168.177C126.479 95.7368 186.197 44.5647 216.599 28.7829C217.794 28.163 219.273 28.5169 220.158 29.5303C317.747 141.292 200.32 211.975 200.32 211.975C200.32 211.975 192.315 216.895 193.965 221.013C195.404 224.603 254.877 305.25 270.1 325.866C271.563 327.847 270.134 330.63 267.671 330.63H175.814C174.891 330.63 174.037 330.231 173.477 329.497C155.029 305.287 120.535 241.081 123.517 168.177Z",
    ]

    func path(in rect: CGRect) -> Path {
        var glyph = Path()
        for d in Self.subpaths { glyph.addPath(SVGPathParser.path(from: d)) }

        let bounds = glyph.boundingRect
        guard bounds.width > 0, bounds.height > 0 else { return glyph }
        let scale = min(rect.width / bounds.width, rect.height / bounds.height)
        let transform = CGAffineTransform(
            a: scale, b: 0, c: 0, d: scale,
            tx: rect.midX - bounds.midX * scale,
            ty: rect.midY - bounds.midY * scale
        )
        return glyph.applying(transform)
    }
}

struct RewinderRMark: View {
    var color: Color = .secondary
    var height: CGFloat = 16

    var body: some View {
        RewinderRShape()
            .fill(color)
            .frame(width: height * 0.82, height: height)
            .accessibilityLabel("Rewinder")
    }
}

enum SVGPathParser {
    static func path(from data: String) -> Path {
        var path = Path()
        let chars = Array(data)
        var i = 0
        var current = CGPoint.zero
        var subpathStart = CGPoint.zero

        func skipSeparators() {
            while i < chars.count {
                let c = chars[i]
                if c == " " || c == "," || c == "\n" || c == "\t" || c == "\r" { i += 1 } else { break }
            }
        }

        func readNumber() -> CGFloat {
            skipSeparators()
            var token = ""
            if i < chars.count, chars[i] == "-" || chars[i] == "+" { token.append(chars[i]); i += 1 }
            while i < chars.count {
                let c = chars[i]
                if c.isNumber || c == "." {
                    token.append(c); i += 1
                } else if c == "e" || c == "E" {
                    token.append(c); i += 1
                    if i < chars.count, chars[i] == "-" || chars[i] == "+" { token.append(chars[i]); i += 1 }
                } else {
                    break
                }
            }
            return CGFloat(Double(token) ?? 0)
        }

        while i < chars.count {
            skipSeparators()
            guard i < chars.count else { break }
            let command = chars[i]
            i += 1
            switch command {
            case "M":
                current = CGPoint(x: readNumber(), y: readNumber())
                subpathStart = current
                path.move(to: current)
            case "L":
                current = CGPoint(x: readNumber(), y: readNumber())
                path.addLine(to: current)
            case "H":
                current.x = readNumber()
                path.addLine(to: current)
            case "V":
                current.y = readNumber()
                path.addLine(to: current)
            case "C":
                let c1 = CGPoint(x: readNumber(), y: readNumber())
                let c2 = CGPoint(x: readNumber(), y: readNumber())
                current = CGPoint(x: readNumber(), y: readNumber())
                path.addCurve(to: current, control1: c1, control2: c2)
            case "Z", "z":
                path.closeSubpath()
                current = subpathStart
            default:
                break
            }
        }
        return path
    }
}
