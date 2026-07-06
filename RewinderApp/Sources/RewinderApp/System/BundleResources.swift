import Foundation

enum BundleResources {
    static func configureEnvironment() {
        guard let resources = Bundle.main.resourceURL else { return }
        let binDir = resources.appendingPathComponent("bin", isDirectory: true)
        let fm = FileManager.default

        let helper = binDir.appendingPathComponent("rewinder-sck-capture")
        if fm.isExecutableFile(atPath: helper.path) {
            setenv("REWINDER_SCK_HELPER_BIN", helper.path, 1)
        }

        let ffmpeg = binDir.appendingPathComponent("ffmpeg")
        if fm.isExecutableFile(atPath: ffmpeg.path) {
            setenv("REWINDER_FFMPEG_BIN", ffmpeg.path, 1)
        }
    }
}
