import CoreAudio
import Foundation

final class AudioRouteWatcher {
    private static let debounceMs = 1_000
    private static let maxEmissionsPerSession = 4

    private let queue = DispatchQueue(label: "rewinder.sck.audio-route", qos: .utility)
    private var lastEchoProne: Bool
    private var emissionCount = 0
    private var debounceTimer: DispatchSourceTimer?
    private var defaultDeviceBlock: AudioObjectPropertyListenerBlock?
    private var dataSourceBlock: AudioObjectPropertyListenerBlock?
    private var dataSourceDevice: AudioDeviceID?
    private var stopped = false

    private var defaultDeviceAddress = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDefaultOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    private var dataSourceAddress = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyDataSource,
        mScope: kAudioDevicePropertyScopeOutput,
        mElement: kAudioObjectPropertyElementMain
    )

    init(initialEchoProne: Bool) {
        lastEchoProne = initialEchoProne
    }

    func start() {
        let block: AudioObjectPropertyListenerBlock = { [weak self] _, _ in
            self?.scheduleProbe()
        }
        defaultDeviceBlock = block
        AudioObjectAddPropertyListenerBlock(
            AudioObjectID(kAudioObjectSystemObject),
            &defaultDeviceAddress,
            queue,
            block
        )
        attachDataSourceListener()
        fputs(
            "phase: audio_route_watch_started echo_prone=\(lastEchoProne ? "1" : "0")\n",
            stderr
        )
        fflush(stderr)
    }

    func stop() {
        queue.sync {
            stopped = true
            debounceTimer?.cancel()
            debounceTimer = nil
        }
        if let defaultDeviceBlock {
            AudioObjectRemovePropertyListenerBlock(
                AudioObjectID(kAudioObjectSystemObject),
                &defaultDeviceAddress,
                queue,
                defaultDeviceBlock
            )
            self.defaultDeviceBlock = nil
        }
        detachDataSourceListener()
    }

    private func attachDataSourceListener() {
        guard let device = AudioOutputProbe.defaultOutputDevice() else { return }
        if device == dataSourceDevice, dataSourceBlock != nil { return }
        detachDataSourceListener()
        let block: AudioObjectPropertyListenerBlock = { [weak self] _, _ in
            self?.scheduleProbe()
        }
        dataSourceBlock = block
        dataSourceDevice = device
        AudioObjectAddPropertyListenerBlock(device, &dataSourceAddress, queue, block)
    }

    private func detachDataSourceListener() {
        if let dataSourceBlock, let dataSourceDevice {
            AudioObjectRemovePropertyListenerBlock(
                dataSourceDevice, &dataSourceAddress, queue, dataSourceBlock
            )
        }
        dataSourceBlock = nil
        dataSourceDevice = nil
    }

    private func scheduleProbe() {
        guard !stopped else { return }
        debounceTimer?.cancel()
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + .milliseconds(Self.debounceMs))
        timer.setEventHandler { [weak self] in
            self?.probeAndEmitIfFlipped()
        }
        timer.resume()
        debounceTimer = timer
    }

    private func probeAndEmitIfFlipped() {
        guard !stopped else { return }
        debounceTimer = nil
        attachDataSourceListener()

        let result = AudioOutputProbe.probe()
        guard result.echoProne != lastEchoProne else { return }
        lastEchoProne = result.echoProne

        guard emissionCount < Self.maxEmissionsPerSession else {
            fputs("phase: audio_route_change_emission_capped\n", stderr)
            fflush(stderr)
            return
        }
        emissionCount += 1
        fputs(
            "phase: audio_output_route_changed transport=\(result.transport) "
                + "source=\(result.source) echo_prone=\(result.echoProne ? "1" : "0")\n",
            stderr
        )
        fflush(stderr)
    }
}
