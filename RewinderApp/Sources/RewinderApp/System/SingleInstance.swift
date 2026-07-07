import AppKit

@MainActor
enum SingleInstance {
    private static var lockFD: Int32 = -1
    private static let activateName = Notification.Name("com.rewinder.app.activate")

    @discardableResult
    static func acquire(onActivate: @escaping @MainActor () -> Void) -> Bool {
        let path = (NSTemporaryDirectory() as NSString)
            .appendingPathComponent("rewinder-app.lock")
        let fd = open(path, O_CREAT | O_RDWR | O_CLOEXEC, 0o644)
        if fd >= 0, flock(fd, LOCK_EX | LOCK_NB) == 0 {
            lockFD = fd
            DistributedNotificationCenter.default().addObserver(
                forName: activateName, object: nil, queue: .main
            ) { _ in
                MainActor.assumeIsolated { onActivate() }
            }
            return true
        }
        if fd >= 0 { close(fd) }
        DistributedNotificationCenter.default().postNotificationName(
            activateName, object: nil, userInfo: nil, deliverImmediately: true
        )
        return false
    }
}
