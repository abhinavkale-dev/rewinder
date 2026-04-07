import Darwin
import Foundation

final class PipeWriter {
    enum OpenStrategy {
        case writerOnlyHandshake
        case bootstrapReadWrite

        var openFlags: Int32 {
            switch self {
            case .writerOnlyHandshake:
                return O_WRONLY | O_NONBLOCK
            case .bootstrapReadWrite:
                return O_RDWR | O_NONBLOCK
            }
        }
    }

    private let handle: FileHandle

    init(
        path: String,
        timeoutNs: UInt64 = 5_000_000_000,
        strategy: OpenStrategy = .writerOnlyHandshake
    ) throws {
        let deadline = DispatchTime.now().uptimeNanoseconds &+ timeoutNs

        while true {
            let fd = open(path, strategy.openFlags)
            if fd >= 0 {
                let flags = fcntl(fd, F_GETFL)
                if flags >= 0 {
                    _ = fcntl(fd, F_SETFL, flags & ~O_NONBLOCK)
                }
                self.handle = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
                return
            }

            let err = errno
            if err != ENXIO && err != ENOENT {
                throw CaptureError.pipeOpenFailed(path, err)
            }

            if DispatchTime.now().uptimeNanoseconds >= deadline {
                throw CaptureError.pipeOpenTimeout(path, err)
            }

            usleep(20_000)
        }
    }

    func write(_ data: Data) throws {
        try handle.write(contentsOf: data)
    }

    func close() {
        try? handle.close()
    }
}
