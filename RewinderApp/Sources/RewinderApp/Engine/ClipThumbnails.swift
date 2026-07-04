import AppKit
import AVFoundation

@MainActor
enum ClipThumbnailCache {
    private static let images = NSCache<NSString, NSImage>()
    private static var durations: [String: Double] = [:]

    static func preview(forPath path: String) async -> (image: NSImage?, durationSecs: Double?) {
        let key = path as NSString
        if let image = images.object(forKey: key) {
            return (image, durations[path])
        }

        let asset = AVURLAsset(url: URL(fileURLWithPath: path))
        let seconds: Double
        do {
            let duration = try await asset.load(.duration)
            seconds = duration.seconds.isFinite ? duration.seconds : 0
            durations[path] = seconds
        } catch {
            return (nil, nil)
        }

        let generator = AVAssetImageGenerator(asset: asset)
        generator.appliesPreferredTrackTransform = true
        generator.maximumSize = CGSize(width: 480, height: 270)

        do {
            let target = CMTime(seconds: max(seconds * 0.1, 0), preferredTimescale: 600)
            let (cgImage, _) = try await generator.image(at: target)
            let image = NSImage(cgImage: cgImage, size: .zero)
            images.setObject(image, forKey: key)
            return (image, seconds)
        } catch {
            return (nil, seconds)
        }
    }
}
