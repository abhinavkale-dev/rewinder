import AudioToolbox
import AVFoundation
import CoreMedia
import CoreVideo
import Darwin
import Foundation
import ScreenCaptureKit

final class CaptureOutput: NSObject, SCStreamOutput, SCStreamDelegate {
    private enum MicAttachRuntimeState {
        case unknown
        case silence
        case live
    }

    private static let micSilenceIntervalMs: Int = 20
    private static let micLiveFrameGraceNs: UInt64 = 750_000_000
    private static let startupNoVideoTimeoutNs: UInt64 = 6_000_000_000
    private static let streamVideoInactivityTimeoutNs: UInt64 = 3_000_000_000
    private static let watchdogTickMs: Int = 500

    private let videoWriter: PipeWriter
    private let targetFps: Int
    private let framePeriodNs: UInt64
    private let requestShutdown: @Sendable (String, Int32) -> Void
    private let systemAudioPipePath: String?
    private let micPipePath: String?
    private let targetAudioSampleRate: Int
    private let targetAudioChannels: Int
    private var systemAudioWriter: PipeWriter?
    private var micWriter: PipeWriter?
    private var micConverter: AVAudioConverter?
    private var micConverterSourceSignature: String?
    private var systemAudioReconnectTimer: DispatchSourceTimer?
    private var micReconnectTimer: DispatchSourceTimer?
    private var micSilenceFillerTimer: DispatchSourceTimer?
    private var captureWatchdogTimer: DispatchSourceTimer?
    private var micAttachState: MicAttachRuntimeState = .unknown
    private var lastMicFrameAtNs: UInt64 = 0
    private var streamStartedAtNs: UInt64 = 0
    private var lastVideoFrameAtNs: UInt64 = 0
    private var micSilenceFrame: Data = Data()
    private var loggedFirstVideo = false
    private var loggedFirstSystemAudio = false
    private var loggedFirstMicAudio = false
    private var loggedSystemAudioPipeConnected = false
    private var loggedMicPipeConnected = false
    private var loggedMicFormat = false
    private var lastMicLevelEmitNs: UInt64 = 0
    private var lastMicRateEmitNs: UInt64 = 0
    private var pendingMicFramesForRate: Int = 0
    private var consecutiveSilenceFills: Int = 0
    private let audioPipeConnectQueue = DispatchQueue(label: "rewinder.audio.pipe.connect", qos: .utility)
    private let audioWriterLock = NSLock()
    private let videoWriteQueue = DispatchQueue(label: "rewinder.video.pipe.write", qos: .userInitiated)
    private var videoPacerTimer: DispatchSourceTimer?
    private var latestFrame: Data?
    private var lastWrittenFrame: Data?
    private var hasFreshFrame = false
    private var framesWritten: UInt64 = 0
    private var duplicatedFrames: UInt64 = 0
    private var skippedFrames: UInt64 = 0
    private var droppedVideoFrames: UInt64 = 0
    private var videoQueueOverflows: UInt64 = 0
    private var lastVideoRateEmitNs: UInt64 = 0
    private var writtenVideoFramesForRate: UInt64 = 0
    private var lastDropEmitNs: UInt64 = 0
    private var lastPacingEmitNs: UInt64 = 0
    private let shutdownStateQueue = DispatchQueue(label: "rewinder.capture.shutdown.state")
    private var shutdownRequested = false
    private var loggedSystemAudioPipeBootstrapped = false
    private var loggedMicPipeBootstrapped = false

    init(
        videoWriter: PipeWriter,
        targetFps: Int,
        requestShutdown: @escaping @Sendable (String, Int32) -> Void,
        systemAudioPipePath: String?,
        micPipePath: String?,
        targetAudioSampleRate: Int,
        targetAudioChannels: Int
    ) {
        self.videoWriter = videoWriter
        self.targetFps = max(targetFps, 1)
        self.framePeriodNs = UInt64(max(1, 1_000_000_000 / max(targetFps, 1)))
        self.requestShutdown = requestShutdown
        self.systemAudioPipePath = systemAudioPipePath
        self.micPipePath = micPipePath
        self.targetAudioSampleRate = max(targetAudioSampleRate, 8_000)
        self.targetAudioChannels = max(targetAudioChannels, 1)
        self.micSilenceFrame = Self.makeSilenceFrame(
            sampleRate: max(targetAudioSampleRate, 8_000),
            channels: max(targetAudioChannels, 1),
            intervalMs: Self.micSilenceIntervalMs
        )
        super.init()
    }

    private static func makeSilenceFrame(sampleRate: Int, channels: Int, intervalMs: Int) -> Data {
        let safeRate = max(sampleRate, 8_000)
        let safeChannels = max(channels, 1)
        let safeIntervalMs = max(intervalMs, 1)
        let frames = max((safeRate * safeIntervalMs) / 1_000, 1)
        let byteCount = frames * safeChannels * MemoryLayout<Float>.size
        return Data(count: byteCount)
    }

    private func writer(isMic: Bool) -> PipeWriter? {
        audioWriterLock.lock()
        defer { audioWriterLock.unlock() }
        return isMic ? micWriter : systemAudioWriter
    }

    private func setWriter(_ nextWriter: PipeWriter?, isMic: Bool) {
        audioWriterLock.lock()
        defer { audioWriterLock.unlock() }
        if isMic {
            micWriter = nextWriter
        } else {
            systemAudioWriter = nextWriter
        }
    }

    private func clearWriter(isMic: Bool) -> PipeWriter? {
        audioWriterLock.lock()
        defer { audioWriterLock.unlock() }
        if isMic {
            let writer = micWriter
            micWriter = nil
            return writer
        }
        let writer = systemAudioWriter
        systemAudioWriter = nil
        return writer
    }

    private func setMicAttachState(_ next: MicAttachRuntimeState) {
        guard next != micAttachState else {
            return
        }

        let previous = micAttachState
        micAttachState = next

        switch next {
        case .silence:
            if previous == .live {
                let nowNs = DispatchTime.now().uptimeNanoseconds
                let ageMs = lastMicFrameAtNs > 0 ? (nowNs &- lastMicFrameAtNs) / 1_000_000 : 0
                fputs("phase: mic_live_frames_lost last_frame_age_ms=\(ageMs)\n", stderr)
                fflush(stderr)
            }
            fputs("phase: mic_silence_filler_active\n", stderr)
            fflush(stderr)
        case .live:
            consecutiveSilenceFills = 0
            fputs("phase: mic_live_frames_detected\n", stderr)
            fflush(stderr)
        case .unknown:
            break
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        stopCaptureInactivityWatchdog()
        let nsError = error as NSError
        let reason = nsError.localizedDescription.replacingOccurrences(of: "\n", with: " ")
        fputs(
            "phase: stream_stopped_error domain=\(nsError.domain) code=\(nsError.code)\n",
            stderr
        )
        fflush(stderr)
        fputs(
            "phase: stream_stop_details domain=\(nsError.domain) code=\(nsError.code) reason=\(reason)\n",
            stderr
        )
        fflush(stderr)
        fputs("ScreenCaptureKit stopped with error: \(error)\n", stderr)
        fflush(stderr)
        let interruptedBySCKService = nsError.domain.contains("SCStreamErrorDomain") && nsError.code == -3805
        let exitCode: Int32 = interruptedBySCKService ? streamInterruptedExitCode : 1
        fputs(
            "phase: stream_stop_classified interrupted=\(interruptedBySCKService ? "true" : "false") exit_code=\(exitCode)\n",
            stderr
        )
        fflush(stderr)
        requestHelperShutdown(
            cause: interruptedBySCKService ? "stream_stopped_interrupted" : "stream_stopped_error",
            exitCode: exitCode
        )
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
        guard sampleBuffer.isValid else {
            return
        }

        if outputType == .screen {
            lastVideoFrameAtNs = DispatchTime.now().uptimeNanoseconds
        }

        switch outputType {
        case .screen:
            do {
                try handleVideo(sampleBuffer)
            } catch {
                fputs("pipe write failed: \(error)\n", stderr)
                fflush(stderr)
                requestHelperShutdown(cause: "video_pipe_write_failed", exitCode: 0)
            }
        case .audio:
            do {
                try handleAudio(sampleBuffer, isMic: false)
            } catch {
                // Keep capture alive if audio writer path is temporarily unavailable.
                fputs("system audio write failed: \(error)\n", stderr)
                fflush(stderr)
            }
        default:
            if #available(macOS 15.0, *), outputType == .microphone {
                do {
                    try handleAudio(sampleBuffer, isMic: true)
                } catch {
                    // Keep capture alive in best-effort mic mode.
                    fputs("microphone write failed: \(error)\n", stderr)
                    fflush(stderr)
                }
            }
        }
    }

    private func handleVideo(_ sampleBuffer: CMSampleBuffer) throws {
        if let status = frameStatus(for: sampleBuffer), status != .complete {
            noteDroppedVideoFrame(reason: "status_\(status.rawValue)")
            return
        }

        guard let pixelBuffer = sampleBuffer.imageBuffer else {
            return
        }

        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

        let pixelFormat = CVPixelBufferGetPixelFormatType(pixelBuffer)
        let packed: Data?
        if pixelFormat == kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange ||
            pixelFormat == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange {
            packed = packNV12(pixelBuffer: pixelBuffer)
        } else {
            packed = packBGRA(pixelBuffer: pixelBuffer)
        }

        guard let packed else {
            return
        }

        publishLatestVideoFrame(packed)
    }

    private func frameStatus(for sampleBuffer: CMSampleBuffer) -> SCFrameStatus? {
        guard
            let attachmentsArray = CMSampleBufferGetSampleAttachmentsArray(
                sampleBuffer,
                createIfNecessary: false
            ) as? [[SCStreamFrameInfo: Any]],
            let attachments = attachmentsArray.first,
            let raw = attachments[.status] as? Int
        else {
            return nil
        }
        return SCFrameStatus(rawValue: raw)
    }

    private func publishLatestVideoFrame(_ frame: Data) {
        videoWriteQueue.async { [self] in
            if hasFreshFrame {
                skippedFrames = skippedFrames.saturatingAdding(1)
            }
            latestFrame = frame
            hasFreshFrame = true
            emitVideoPacingMetricsIfNeededLocked()
        }
    }

    private func startVideoPacerLocked() {
        guard videoPacerTimer == nil else {
            return
        }

        let timer = DispatchSource.makeTimerSource(queue: videoWriteQueue)
        let periodNs = max(Int(framePeriodNs), 1)
        timer.schedule(
            deadline: .now() + .nanoseconds(periodNs),
            repeating: .nanoseconds(periodNs)
        )
        timer.setEventHandler { [weak self] in
            self?.tickVideoPacerLocked()
        }
        videoPacerTimer = timer
        timer.resume()
    }

    private func stopVideoPacerLocked() {
        videoPacerTimer?.setEventHandler {}
        videoPacerTimer?.cancel()
        videoPacerTimer = nil
    }

    private func tickVideoPacerLocked() {
        let frameToWrite: Data
        if hasFreshFrame, let latestFrame {
            frameToWrite = latestFrame
            hasFreshFrame = false
            lastWrittenFrame = latestFrame
        } else if let lastWrittenFrame {
            frameToWrite = lastWrittenFrame
            duplicatedFrames = duplicatedFrames.saturatingAdding(1)
        } else {
            emitVideoPacingMetricsIfNeededLocked()
            return
        }

        do {
            try videoWriter.write(frameToWrite)
        } catch {
            fputs("pipe write failed: \(error)\n", stderr)
            fflush(stderr)
            requestHelperShutdown(cause: "video_pipe_write_failed", exitCode: 0)
            return
        }

        framesWritten = framesWritten.saturatingAdding(1)
        if !loggedFirstVideo {
            loggedFirstVideo = true
            fputs("first video frame delivered\n", stderr)
            fflush(stderr)
        }

        writtenVideoFramesForRate = writtenVideoFramesForRate.saturatingAdding(1)
        emitVideoRateIfNeededLocked()
        emitVideoPacingMetricsIfNeededLocked()
    }

    private func noteDroppedVideoFrame(reason: String) {
        videoWriteQueue.async { [self] in
            droppedVideoFrames = droppedVideoFrames.saturatingAdding(1)
            emitVideoDropMarkersLocked(reason: reason)
        }
    }

    private func emitVideoDropMarkersLocked(reason: String) {
        let nowNs = DispatchTime.now().uptimeNanoseconds
        if lastDropEmitNs == 0 {
            lastDropEmitNs = nowNs
        }
        if nowNs > lastDropEmitNs && (nowNs - lastDropEmitNs) >= 300_000_000 {
            fputs("video_frame_drop_total=\(droppedVideoFrames) reason=\(reason)\n", stderr)
            fflush(stderr)
            fputs("video_queue_overflow_count=\(videoQueueOverflows)\n", stderr)
            fflush(stderr)
            lastDropEmitNs = nowNs
        }
    }

    private func emitVideoRateIfNeededLocked() {
        let nowNs = DispatchTime.now().uptimeNanoseconds
        if lastVideoRateEmitNs == 0 {
            lastVideoRateEmitNs = nowNs
            return
        }

        let elapsedNs = nowNs > lastVideoRateEmitNs ? (nowNs - lastVideoRateEmitNs) : 0
        if elapsedNs >= 1_000_000_000 {
            let elapsedSecs = Double(elapsedNs) / 1_000_000_000.0
            let fps = elapsedSecs > 0
                ? Double(writtenVideoFramesForRate) / elapsedSecs
                : 0
            fputs(String(format: "video_output_fps=%.2f\n", fps), stderr)
            fflush(stderr)
            writtenVideoFramesForRate = 0
            lastVideoRateEmitNs = nowNs
        }
    }

    private func emitVideoPacingMetricsIfNeededLocked() {
        let nowNs = DispatchTime.now().uptimeNanoseconds
        if lastPacingEmitNs == 0 {
            lastPacingEmitNs = nowNs
            return
        }
        let elapsedNs = nowNs > lastPacingEmitNs ? (nowNs - lastPacingEmitNs) : 0
        if elapsedNs < 1_000_000_000 {
            return
        }

        fputs("video_frame_dup_total=\(duplicatedFrames)\n", stderr)
        fflush(stderr)
        fputs("video_frame_skip_total=\(skippedFrames)\n", stderr)
        fflush(stderr)
        lastPacingEmitNs = nowNs
    }

    private func packNV12(pixelBuffer: CVPixelBuffer) -> Data? {
        guard CVPixelBufferGetPlaneCount(pixelBuffer) >= 2 else {
            return nil
        }
        guard
            let yBase = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 0),
            let uvBase = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 1)
        else {
            return nil
        }

        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        let yStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 0)
        let uvStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 1)
        let yRows = min(height, CVPixelBufferGetHeightOfPlane(pixelBuffer, 0))
        let uvRows = min(height / 2, CVPixelBufferGetHeightOfPlane(pixelBuffer, 1))
        let yCopyWidth = width
        let uvCopyWidth = width
        let yCopyBytes = yCopyWidth * yRows
        let uvCopyBytes = uvCopyWidth * uvRows

        var out = Data(count: yCopyBytes + uvCopyBytes)
        out.withUnsafeMutableBytes { dstRaw in
            guard let dstBase = dstRaw.baseAddress else {
                return
            }

            for row in 0..<yRows {
                let src = yBase.advanced(by: row * yStride)
                let dst = dstBase.advanced(by: row * yCopyWidth)
                memcpy(dst, src, yCopyWidth)
            }

            let uvDstBase = dstBase.advanced(by: yCopyBytes)
            for row in 0..<uvRows {
                let src = uvBase.advanced(by: row * uvStride)
                let dst = uvDstBase.advanced(by: row * uvCopyWidth)
                memcpy(dst, src, uvCopyWidth)
            }
        }
        return out
    }

    private func packBGRA(pixelBuffer: CVPixelBuffer) -> Data? {
        guard let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else {
            return nil
        }

        let frameWidth = CVPixelBufferGetWidth(pixelBuffer)
        let frameHeight = CVPixelBufferGetHeight(pixelBuffer)
        let bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)
        let expectedRow = frameWidth * 4

        if bytesPerRow == expectedRow {
            return Data(bytes: baseAddress, count: bytesPerRow * frameHeight)
        }

        var contiguous = Data(count: expectedRow * frameHeight)
        contiguous.withUnsafeMutableBytes { dstRaw in
            guard let dstBase = dstRaw.baseAddress else {
                return
            }
            for row in 0..<frameHeight {
                let src = baseAddress.advanced(by: row * bytesPerRow)
                let dst = dstBase.advanced(by: row * expectedRow)
                memcpy(dst, src, expectedRow)
            }
        }
        return contiguous
    }

    private func handleAudio(_ sampleBuffer: CMSampleBuffer, isMic: Bool) throws {
        guard let writer = resolvedWriter(isMic: isMic) else {
            requestAudioWriterConnect(isMic: isMic, connectTimeoutNs: 50_000_000)
            return
        }

        guard let formatDescription = CMSampleBufferGetFormatDescription(sampleBuffer),
              let asbdPointer = CMAudioFormatDescriptionGetStreamBasicDescription(formatDescription)
        else {
            return
        }

        let asbd = asbdPointer.pointee
        let frameCount = CMSampleBufferGetNumSamples(sampleBuffer)
        if frameCount <= 0 {
            return
        }

        var requiredSize = 0
        var blockBuffer: CMBlockBuffer?
        let sizeStatus = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sampleBuffer,
            bufferListSizeNeededOut: &requiredSize,
            bufferListOut: nil,
            bufferListSize: 0,
            blockBufferAllocator: nil,
            blockBufferMemoryAllocator: nil,
            flags: kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
            blockBufferOut: &blockBuffer
        )
        if sizeStatus != noErr || requiredSize == 0 {
            return
        }

        let rawPointer = UnsafeMutableRawPointer.allocate(
            byteCount: requiredSize,
            alignment: MemoryLayout<AudioBufferList>.alignment
        )
        defer { rawPointer.deallocate() }

        let audioBufferListPointer = rawPointer.bindMemory(to: AudioBufferList.self, capacity: 1)
        let status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sampleBuffer,
            bufferListSizeNeededOut: nil,
            bufferListOut: audioBufferListPointer,
            bufferListSize: requiredSize,
            blockBufferAllocator: nil,
            blockBufferMemoryAllocator: nil,
            flags: kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
            blockBufferOut: &blockBuffer
        )

        if status != noErr {
            return
        }

        let audioBuffers = UnsafeMutableAudioBufferListPointer(audioBufferListPointer)
        let channels = max(Int(asbd.mChannelsPerFrame), 1)
        let isFloat = (asbd.mFormatFlags & kAudioFormatFlagIsFloat) != 0
        let isNonInterleaved = (asbd.mFormatFlags & kAudioFormatFlagIsNonInterleaved) != 0
        let bitsPerChannel = Int(asbd.mBitsPerChannel)

        var interleaved = convertToInterleavedFloat32(
            buffers: audioBuffers,
            frameCount: frameCount,
            channels: channels,
            isFloat: isFloat,
            isNonInterleaved: isNonInterleaved,
            bitsPerChannel: bitsPerChannel
        )

        guard var interleavedData = interleaved else {
            return
        }

        if isMic {
            if !loggedMicFormat {
                loggedMicFormat = true
                fputs(
                    "mic source format: sample_rate=\(Int(asbd.mSampleRate)) channels=\(channels) float=\(isFloat) interleaved=\(!isNonInterleaved) bits=\(bitsPerChannel)\n",
                    stderr
                )
                fputs(
                    "mic target format: sample_rate=\(targetAudioSampleRate) channels=\(targetAudioChannels) float=true interleaved=true bits=32\n",
                    stderr
                )
                fflush(stderr)
            }

            if let normalized = normalizeMicToTargetFormat(
                interleaved: interleavedData,
                sourceSampleRate: asbd.mSampleRate,
                sourceChannels: channels,
                frameCount: frameCount
            ) {
                interleavedData = normalized
            } else {
                fputs("microphone conversion failed; dropping current mic buffer\n", stderr)
                fflush(stderr)
                return
            }
        }

        do {
            try writer.write(interleavedData)
        } catch {
            handleAudioWriterFailure(isMic: isMic, error: error)
            throw error
        }

        if isMic {
            lastMicFrameAtNs = DispatchTime.now().uptimeNanoseconds
            setMicAttachState(.live)
            emitMicLevelDbfs(interleavedData)
            emitMicSamplesPerSecond(interleavedData)
            if !loggedFirstMicAudio {
                loggedFirstMicAudio = true
                fputs("first microphone audio frame delivered\n", stderr)
                fflush(stderr)
            }
        } else {
            if !loggedFirstSystemAudio {
                loggedFirstSystemAudio = true
                fputs("first system audio frame delivered\n", stderr)
                fflush(stderr)
            }
        }
    }

    private func convertToInterleavedFloat32(
        buffers: UnsafeMutableAudioBufferListPointer,
        frameCount: Int,
        channels: Int,
        isFloat: Bool,
        isNonInterleaved: Bool,
        bitsPerChannel: Int
    ) -> Data? {
        var output = Data(count: frameCount * channels * MemoryLayout<Float>.size)

        let didConvert = output.withUnsafeMutableBytes { rawDst -> Bool in
            let dst = rawDst.bindMemory(to: Float.self)

            func readSample(pointer: UnsafeRawPointer, index: Int) -> Float {
                if isFloat && bitsPerChannel == 32 {
                    return pointer.assumingMemoryBound(to: Float.self)[index]
                }
                if !isFloat && bitsPerChannel == 16 {
                    let sample = pointer.assumingMemoryBound(to: Int16.self)[index]
                    return Float(sample) / Float(Int16.max)
                }
                return 0
            }

            if isNonInterleaved {
                if buffers.count < channels {
                    return false
                }

                for frame in 0..<frameCount {
                    for channel in 0..<channels {
                        guard let src = buffers[channel].mData else {
                            return false
                        }
                        dst[frame * channels + channel] = readSample(pointer: src, index: frame)
                    }
                }
                return true
            }

            guard let src = buffers.first?.mData else {
                return false
            }

            let totalSamples = frameCount * channels
            for sampleIndex in 0..<totalSamples {
                dst[sampleIndex] = readSample(pointer: src, index: sampleIndex)
            }

            return true
        }

        return didConvert ? output : nil
    }

    private func normalizeMicToTargetFormat(
        interleaved: Data,
        sourceSampleRate: Float64,
        sourceChannels: Int,
        frameCount: Int
    ) -> Data? {
        let targetSampleRate = Float64(targetAudioSampleRate)
        let targetChannels = targetAudioChannels
        let needsConversion = sourceChannels != targetChannels || abs(sourceSampleRate - targetSampleRate) > 1.0
        if !needsConversion {
            return interleaved
        }

        guard
            let sourceFormat = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: sourceSampleRate,
                channels: AVAudioChannelCount(max(sourceChannels, 1)),
                interleaved: true
            ),
            let targetFormat = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: targetSampleRate,
                channels: AVAudioChannelCount(targetChannels),
                interleaved: true
            )
        else {
            return nil
        }

        let sourceSignature = "\(Int(sourceSampleRate))hz-\(sourceChannels)ch"
        if micConverter == nil || micConverterSourceSignature != sourceSignature {
            micConverter = AVAudioConverter(from: sourceFormat, to: targetFormat)
            micConverterSourceSignature = sourceSignature
            fputs(
                "mic converter configured: \(sourceSignature) -> \(targetAudioSampleRate)hz-\(targetChannels)ch\n",
                stderr
            )
            fflush(stderr)
        }

        guard let converter = micConverter else {
            return nil
        }

        let safeFrameCount = max(frameCount, 1)
        guard let sourceBuffer = AVAudioPCMBuffer(
            pcmFormat: sourceFormat,
            frameCapacity: AVAudioFrameCount(safeFrameCount)
        ) else {
            return nil
        }
        sourceBuffer.frameLength = AVAudioFrameCount(safeFrameCount)

        let sourceBufferList = UnsafeMutableAudioBufferListPointer(sourceBuffer.mutableAudioBufferList)
        guard let sourceAudioBuffer = sourceBufferList.first,
              let sourceDataPtr = sourceAudioBuffer.mData
        else {
            return nil
        }
        interleaved.withUnsafeBytes { raw in
            guard let srcPtr = raw.baseAddress else {
                return
            }
            let copySize = min(Int(sourceAudioBuffer.mDataByteSize), raw.count)
            memcpy(sourceDataPtr, srcPtr, copySize)
        }

        let outputFrameCapacity = AVAudioFrameCount(
            max(1, Int(ceil(Double(safeFrameCount) * targetSampleRate / max(sourceSampleRate, 1.0)))) + 64
        )
        guard let outputBuffer = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: outputFrameCapacity) else {
            return nil
        }

        var didProvideInput = false
        var conversionError: NSError?
        let conversionStatus = converter.convert(to: outputBuffer, error: &conversionError) { _, outStatus in
            if didProvideInput {
                outStatus.pointee = .noDataNow
                return nil
            }
            didProvideInput = true
            outStatus.pointee = .haveData
            return sourceBuffer
        }

        if conversionStatus == .error || conversionError != nil {
            if let conversionError {
                fputs("mic conversion error: \(conversionError)\n", stderr)
                fflush(stderr)
            }
            return nil
        }

        let outputBufferList = UnsafeMutableAudioBufferListPointer(outputBuffer.mutableAudioBufferList)
        guard let outputAudioBuffer = outputBufferList.first,
              let outputDataPtr = outputAudioBuffer.mData
        else {
            return nil
        }
        let outputByteCount =
            Int(outputBuffer.frameLength) * targetChannels * MemoryLayout<Float>.size
        return Data(bytes: outputDataPtr, count: outputByteCount)
    }

    private func emitMicLevelDbfs(_ interleaved: Data) {
        let now = DispatchTime.now().uptimeNanoseconds
        if now &- lastMicLevelEmitNs < 500_000_000 {
            return
        }
        lastMicLevelEmitNs = now

        let sampleCount = interleaved.count / MemoryLayout<Float>.size
        if sampleCount == 0 {
            return
        }

        let rms: Float = interleaved.withUnsafeBytes { raw -> Float in
            guard let base = raw.baseAddress?.assumingMemoryBound(to: Float.self) else {
                return 0
            }
            var sumSquares: Float = 0
            for idx in 0..<sampleCount {
                let sample = base[idx]
                sumSquares += sample * sample
            }
            return sqrtf(sumSquares / Float(sampleCount))
        }

        let floor: Float = 0.000_000_1
        let dbfs = 20.0 * log10f(max(rms, floor))
        fputs(String(format: "mic_level_dbfs=%.1f\n", dbfs), stderr)
        fflush(stderr)
    }

    private func emitMicSamplesPerSecond(_ interleaved: Data) {
        let bytesPerFrame = max(targetAudioChannels, 1) * MemoryLayout<Float>.size
        if bytesPerFrame <= 0 {
            return
        }
        let frames = interleaved.count / bytesPerFrame
        if frames <= 0 {
            return
        }

        pendingMicFramesForRate += frames
        let now = DispatchTime.now().uptimeNanoseconds
        if lastMicRateEmitNs == 0 {
            lastMicRateEmitNs = now
            return
        }

        let elapsedNs = now &- lastMicRateEmitNs
        if elapsedNs < 1_000_000_000 {
            return
        }

        let elapsedSec = Double(elapsedNs) / 1_000_000_000.0
        if elapsedSec <= 0 {
            return
        }
        let samplesPerSec = Int((Double(pendingMicFramesForRate) / elapsedSec).rounded())
        fputs("mic_samples_per_sec=\(samplesPerSec)\n", stderr)
        fflush(stderr)
        pendingMicFramesForRate = 0
        lastMicRateEmitNs = now
    }

    func closeWriters() {
        beginShutdown()
        videoWriter.close()
        clearWriter(isMic: false)?.close()
        clearWriter(isMic: true)?.close()
    }

    func beginShutdown() {
        shutdownStateQueue.sync {
            shutdownRequested = true
        }
        stopCaptureInactivityWatchdog()
        stopSystemAudioPipeReconnectLoop()
        stopMicPipeReconnectLoop()
        stopMicSilenceFillerLoop()
        let stopGroup = DispatchGroup()
        stopGroup.enter()
        videoWriteQueue.async { [self] in
            stopVideoPacerLocked()
            stopGroup.leave()
        }
        _ = stopGroup.wait(timeout: .now() + .milliseconds(250))
    }

    func markStreamStarted() {
        let nowNs = DispatchTime.now().uptimeNanoseconds
        streamStartedAtNs = nowNs
        lastVideoFrameAtNs = 0
        videoWriteQueue.async { [self] in
            latestFrame = nil
            lastWrittenFrame = nil
            hasFreshFrame = false
            framesWritten = 0
            duplicatedFrames = 0
            skippedFrames = 0
            droppedVideoFrames = 0
            videoQueueOverflows = 0
            writtenVideoFramesForRate = 0
            lastVideoRateEmitNs = 0
            lastDropEmitNs = 0
            lastPacingEmitNs = 0
            startVideoPacerLocked()
        }
    }

    func startCaptureInactivityWatchdog(on queue: DispatchQueue) {
        guard captureWatchdogTimer == nil else {
            return
        }

        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(
            deadline: .now() + .milliseconds(Self.watchdogTickMs),
            repeating: .milliseconds(Self.watchdogTickMs)
        )
        timer.setEventHandler { [weak self] in
            self?.evaluateCaptureInactivityWatchdog()
        }
        captureWatchdogTimer = timer
        timer.resume()
    }

    private func stopCaptureInactivityWatchdog() {
        captureWatchdogTimer?.setEventHandler {}
        captureWatchdogTimer?.cancel()
        captureWatchdogTimer = nil
    }

    private func evaluateCaptureInactivityWatchdog() {
        guard streamStartedAtNs > 0 else {
            return
        }

        let nowNs = DispatchTime.now().uptimeNanoseconds
        let elapsedSinceStart = nowNs &- streamStartedAtNs
        if lastVideoFrameAtNs == 0 {
            if elapsedSinceStart >= Self.startupNoVideoTimeoutNs {
                let elapsedMs = elapsedSinceStart / 1_000_000
                fputs(
                    "phase: stream_inactive_watchdog_triggered reason=no_video_after_start elapsed_ms=\(elapsedMs)\n",
                    stderr
                )
                fflush(stderr)
                fputs(
                    "phase: stream_stop_classified interrupted=true exit_code=\(streamInterruptedExitCode) reason=watchdog_no_video\n",
                    stderr
                )
                fflush(stderr)
                requestHelperShutdown(
                    cause: "watchdog_no_video",
                    exitCode: streamInterruptedExitCode
                )
            }
            return
        }

        let elapsedSinceVideo = nowNs &- lastVideoFrameAtNs
        if elapsedSinceVideo >= Self.streamVideoInactivityTimeoutNs {
            let elapsedMs = elapsedSinceVideo / 1_000_000
            fputs(
                "phase: stream_inactive_watchdog_triggered reason=video_stalled elapsed_ms=\(elapsedMs)\n",
                stderr
            )
            fflush(stderr)
            fputs(
                "phase: stream_stop_classified interrupted=true exit_code=\(streamInterruptedExitCode) reason=watchdog_video_stalled\n",
                stderr
            )
            fflush(stderr)
            requestHelperShutdown(
                cause: "watchdog_video_stalled",
                exitCode: streamInterruptedExitCode
            )
        }
    }

    func primeSystemAudioPipe(sampleRate: Int, channels: Int) {
        _ = bootstrapAudioWriter(isMic: false, connectTimeoutNs: 2_000_000_000)
        let frames = max(sampleRate / 50, 1) // 20ms of silence
        let byteCount = frames * max(channels, 1) * MemoryLayout<Float>.size
        let silence = Data(count: byteCount)
        do {
            if let systemAudioWriter = resolvedWriter(isMic: false) {
                try systemAudioWriter.write(silence)
            }
        } catch {
            fputs("system audio pipe priming failed: \(error)\n", stderr)
            fflush(stderr)
        }
    }

    func primeMicPipe(sampleRate: Int, channels: Int) {
        _ = bootstrapAudioWriter(isMic: true, connectTimeoutNs: 500_000_000)

        let frames = max(sampleRate / 50, 1) // 20ms of silence
        let byteCount = frames * max(channels, 1) * MemoryLayout<Float>.size
        let silence = Data(count: byteCount)
        micSilenceFrame = silence
        do {
            if let micWriter = resolvedWriter(isMic: true) {
                try micWriter.write(silence)
                setMicAttachState(.silence)
            }
        } catch {
            fputs("microphone pipe priming failed: \(error)\n", stderr)
            fflush(stderr)
        }
    }

    func startMicSilenceFillerLoop(on queue: DispatchQueue) {
        guard micPipePath != nil else {
            return
        }
        guard micSilenceFillerTimer == nil else {
            return
        }

        if micSilenceFrame.isEmpty {
            micSilenceFrame = Self.makeSilenceFrame(
                sampleRate: targetAudioSampleRate,
                channels: targetAudioChannels,
                intervalMs: Self.micSilenceIntervalMs
            )
        }

        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(
            deadline: .now() + .milliseconds(80),
            repeating: .milliseconds(Self.micSilenceIntervalMs)
        )
        timer.setEventHandler { [weak self] in
            self?.writeMicSilenceIfNeeded()
        }
        micSilenceFillerTimer = timer
        timer.resume()
    }

    private func stopMicSilenceFillerLoop() {
        micSilenceFillerTimer?.setEventHandler {}
        micSilenceFillerTimer?.cancel()
        micSilenceFillerTimer = nil
    }

    private func writeMicSilenceIfNeeded() {
        guard micPipePath != nil else {
            return
        }
        guard let writer = resolvedWriter(isMic: true) else {
            requestAudioWriterConnect(isMic: true, connectTimeoutNs: 50_000_000)
            return
        }

        let nowNs = DispatchTime.now().uptimeNanoseconds
        if lastMicFrameAtNs > 0 {
            let elapsed = nowNs >= lastMicFrameAtNs ? (nowNs - lastMicFrameAtNs) : UInt64.max
            if elapsed <= Self.micLiveFrameGraceNs {
                return
            }
        }

        setMicAttachState(.silence)
        consecutiveSilenceFills += 1
        if consecutiveSilenceFills == 150 || (consecutiveSilenceFills > 150 && consecutiveSilenceFills % 150 == 0) {
            fputs("phase: mic_sustained_silence_detected fills=\(consecutiveSilenceFills)\n", stderr)
            fflush(stderr)
        }
        if micSilenceFrame.isEmpty {
            micSilenceFrame = Self.makeSilenceFrame(
                sampleRate: targetAudioSampleRate,
                channels: targetAudioChannels,
                intervalMs: Self.micSilenceIntervalMs
            )
        }
        do {
            try writer.write(micSilenceFrame)
        } catch {
            handleAudioWriterFailure(isMic: true, error: error)
        }
    }

    func startSystemAudioPipeReconnectLoop() {
        guard systemAudioPipePath != nil else {
            return
        }
        guard resolvedWriter(isMic: false) == nil, systemAudioReconnectTimer == nil else {
            return
        }

        let timer = DispatchSource.makeTimerSource(queue: audioPipeConnectQueue)
        timer.schedule(deadline: .now() + .milliseconds(120), repeating: .milliseconds(250))
        timer.setEventHandler { [weak self] in
            guard let self else {
                timer.cancel()
                return
            }
            if self.resolvedWriter(isMic: false) != nil {
                timer.cancel()
                self.systemAudioReconnectTimer = nil
                return
            }
            _ = self.connectAudioWriterIfNeededLocked(isMic: false, connectTimeoutNs: 50_000_000)
            if self.resolvedWriter(isMic: false) != nil {
                timer.cancel()
                self.systemAudioReconnectTimer = nil
            }
        }
        systemAudioReconnectTimer = timer
        timer.resume()
    }

    private func stopSystemAudioPipeReconnectLoop() {
        systemAudioReconnectTimer?.setEventHandler {}
        systemAudioReconnectTimer?.cancel()
        systemAudioReconnectTimer = nil
    }

    func startMicPipeReconnectLoop() {
        guard micPipePath != nil else {
            return
        }
        guard resolvedWriter(isMic: true) == nil, micReconnectTimer == nil else {
            return
        }

        let timer = DispatchSource.makeTimerSource(queue: audioPipeConnectQueue)
        timer.schedule(deadline: .now() + .milliseconds(120), repeating: .milliseconds(250))
        timer.setEventHandler { [weak self] in
            guard let self else {
                timer.cancel()
                return
            }
            if self.resolvedWriter(isMic: true) != nil {
                timer.cancel()
                self.micReconnectTimer = nil
                return
            }
            _ = self.connectAudioWriterIfNeededLocked(isMic: true, connectTimeoutNs: 50_000_000)
            if self.resolvedWriter(isMic: true) != nil {
                timer.cancel()
                self.micReconnectTimer = nil
            }
        }
        micReconnectTimer = timer
        timer.resume()
    }

    private func stopMicPipeReconnectLoop() {
        micReconnectTimer?.setEventHandler {}
        micReconnectTimer?.cancel()
        micReconnectTimer = nil
    }

    private func requestHelperShutdown(cause: String, exitCode: Int32) {
        let shouldRequest = shutdownStateQueue.sync { () -> Bool in
            if shutdownRequested {
                return false
            }
            shutdownRequested = true
            return true
        }
        if shouldRequest {
            requestShutdown(cause, exitCode)
        }
    }

    func ingestMicrophoneSampleBuffer(_ sampleBuffer: CMSampleBuffer) {
        do {
            try handleAudio(sampleBuffer, isMic: true)
        } catch {
            fputs("microphone sample write failed: \(error)\n", stderr)
            fflush(stderr)
        }
    }

    private func resolvedWriter(isMic: Bool) -> PipeWriter? {
        writer(isMic: isMic)
    }

    @discardableResult
    private func bootstrapAudioWriter(isMic: Bool, connectTimeoutNs: UInt64) -> Bool {
        audioPipeConnectQueue.sync {
            connectAudioWriterIfNeededLocked(
                isMic: isMic,
                connectTimeoutNs: connectTimeoutNs,
                emitBootstrapMarker: true
            )
        }
    }

    private func requestAudioWriterConnect(isMic: Bool, connectTimeoutNs: UInt64) {
        audioPipeConnectQueue.async { [weak self] in
            guard let self else {
                return
            }
            _ = self.connectAudioWriterIfNeededLocked(isMic: isMic, connectTimeoutNs: connectTimeoutNs)
        }
    }

    private func handleAudioWriterFailure(isMic: Bool, error: Error) {
        let prefix = isMic ? "microphone" : "system audio"
        fputs("\(prefix) write failed: \(error)\n", stderr)
        fflush(stderr)
        clearWriter(isMic: isMic)?.close()
        if isMic {
            startMicPipeReconnectLoop()
        } else {
            startSystemAudioPipeReconnectLoop()
        }
    }

    @discardableResult
    private func connectAudioWriterIfNeededLocked(
        isMic: Bool,
        connectTimeoutNs: UInt64,
        emitBootstrapMarker: Bool = false
    ) -> Bool {
        if resolvedWriter(isMic: isMic) != nil {
            return true
        }

        let pipePath = isMic ? micPipePath : systemAudioPipePath
        guard let pipePath else {
            return false
        }
        do {
            let newWriter = try PipeWriter(
                path: pipePath,
                timeoutNs: connectTimeoutNs,
                strategy: .writerOnlyHandshake
            )
            setWriter(newWriter, isMic: isMic)
            if isMic {
                if emitBootstrapMarker && !loggedMicPipeBootstrapped {
                    loggedMicPipeBootstrapped = true
                    fputs("phase: mic_audio_pipe_bootstrapped\n", stderr)
                    fflush(stderr)
                }
                if !loggedMicPipeConnected {
                    loggedMicPipeConnected = true
                    fputs("phase: mic_audio_pipe_connected\n", stderr)
                    fflush(stderr)
                }
            } else {
                if emitBootstrapMarker && !loggedSystemAudioPipeBootstrapped {
                    loggedSystemAudioPipeBootstrapped = true
                    fputs("phase: system_audio_pipe_bootstrapped\n", stderr)
                    fflush(stderr)
                }
                if !loggedSystemAudioPipeConnected {
                    loggedSystemAudioPipeConnected = true
                    fputs("phase: system_audio_pipe_connected\n", stderr)
                    fflush(stderr)
                }
            }
            return true
        } catch {
            if emitBootstrapMarker {
                let pipeName = isMic ? "mic_audio" : "system_audio"
                let detail = String(describing: error).replacingOccurrences(of: "\n", with: " ")
                fputs("phase: audio_pipe_bootstrap_failed pipe=\(pipeName) error=\(detail)\n", stderr)
                fflush(stderr)
            }
            return false
        }
    }
}
