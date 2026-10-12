import Foundation
import CoreAudio
import AVFAudio

private typealias AudioOutput = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<Float>, UInt32) -> Void

private func property<T>(_ object: AudioObjectID, _ selector: AudioObjectPropertySelector, _ value: inout T) -> OSStatus {
    var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var size = UInt32(MemoryLayout<T>.size)
    return withUnsafeMutablePointer(to: &value) { AudioObjectGetPropertyData(object, &address, 0, nil, &size, $0) }
}

@available(macOS 14.2, *)
private func audioProcesses() -> [AudioObjectID] {
    var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyProcessObjectList, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size) == 0 else { return [] }
    var objects = [AudioObjectID](repeating: 0, count: Int(size) / MemoryLayout<AudioObjectID>.size)
    let status = objects.withUnsafeMutableBytes { AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, $0.baseAddress!) }
    return status == 0 ? objects : []
}

private func inputChannels(_ device: AudioObjectID) -> UInt32? {
    var address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyStreamConfiguration, mScope: kAudioObjectPropertyScopeInput, mElement: kAudioObjectPropertyElementMain)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(device, &address, 0, nil, &size) == 0, size >= MemoryLayout<UInt32>.size else { return nil }
    let data = UnsafeMutableRawPointer.allocate(byteCount: max(Int(size), MemoryLayout<AudioBufferList>.size), alignment: MemoryLayout<AudioBufferList>.alignment)
    defer { data.deallocate() }
    guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, data) == 0 else { return nil }
    return UnsafeMutableAudioBufferListPointer(data.assumingMemoryBound(to: AudioBufferList.self)).reduce(0) { $0 + $1.mNumberChannels }
}
// Core Audio attributes Chrome's output to its helper, which ScreenCaptureKit omits.
private func appProcesses(_ pid: Int32) -> Set<Int32> {
    let command = Process()
    command.executableURL = URL(fileURLWithPath: "/bin/ps")
    command.arguments = ["-axo", "pid=,ppid="]
    let pipe = Pipe()
    command.standardOutput = pipe
    do { try command.run() } catch { return [pid] }
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    command.waitUntilExit()
    let parents = String(decoding: data, as: UTF8.self).split(separator: "\n").compactMap { row -> (Int32, Int32)? in
        let columns = row.split(whereSeparator: { $0.isWhitespace })
        guard columns.count == 2, let child = Int32(columns[0]), let parent = Int32(columns[1]) else { return nil }
        return (child, parent)
    }
    var included: Set<Int32> = [pid]
    var previous = 0
    while previous != included.count {
        previous = included.count
        for (child, parent) in parents where included.contains(parent) { included.insert(child) }
    }
    return included
}

@available(macOS 14.2, *)
private final class AppAudioTap {
    var tap: AudioObjectID = 0
    var device: AudioObjectID = 0
    var io: AudioDeviceIOProcID?

    deinit {
        if let io = io { AudioDeviceStop(device, io); AudioDeviceDestroyIOProcID(device, io) }
        if device != 0 { AudioHardwareDestroyAggregateDevice(device) }
        if tap != 0 { AudioHardwareDestroyProcessTap(tap) }
    }

    func start(_ pid: Int32, _ output: @escaping AudioOutput, _ context: UnsafeMutableRawPointer?) -> Bool {
        // shortcut: snapshots processes/device format; add HAL listeners for late helpers or live format changes.
        let family = appProcesses(pid)
        let objects = audioProcesses().filter { object in
            var process: pid_t = 0
            return property(object, kAudioProcessPropertyPID, &process) == 0 && family.contains(process)
        }
        guard !objects.isEmpty else { return false }
        let description = CATapDescription(stereoMixdownOfProcesses: objects)
        if #available(macOS 26.0, *) { description.isProcessRestoreEnabled = true }
        description.isPrivate = true
        description.muteBehavior = .unmuted
        guard AudioHardwareCreateProcessTap(description, &tap) == 0 else { return false }
        var format = AudioStreamBasicDescription()
        guard property(tap, kAudioTapPropertyFormat, &format) == 0,
              format.mSampleRate > 0, format.mChannelsPerFrame == 2,
              format.mBitsPerChannel == 32, format.mFormatFlags & kAudioFormatFlagIsFloat != 0 else { return false }

        var outputDevice: AudioObjectID = 0
        guard property(AudioObjectID(kAudioObjectSystemObject), kAudioHardwarePropertyDefaultOutputDevice, &outputDevice) == 0, outputDevice != 0 else { return false }
        var outputUID: CFString = "" as CFString
        guard property(outputDevice, kAudioDevicePropertyDeviceUID, &outputUID) == 0 else { return false }
        guard let clockInputs = inputChannels(outputDevice) else { return false }
        // The output subdevice supplies the clock; its input is never forwarded.
        let aggregate: [String: Any] = [
            kAudioAggregateDeviceNameKey: "Screenshare app audio",
            kAudioAggregateDeviceUIDKey: UUID().uuidString,
            kAudioAggregateDeviceIsPrivateKey: true,
            kAudioAggregateDeviceMainSubDeviceKey: outputUID,
            kAudioAggregateDeviceSubDeviceListKey: [[kAudioSubDeviceUIDKey: outputUID]],
            kAudioAggregateDeviceTapAutoStartKey: true,
            kAudioAggregateDeviceTapListKey: [[kAudioSubTapUIDKey: description.uuid.uuidString, kAudioSubTapDriftCompensationKey: true]],
        ]
        guard AudioHardwareCreateAggregateDevice(aggregate as CFDictionary, &device) == 0 else { return false }
        var sampleRate: Float64 = 0
        guard property(device, kAudioDevicePropertyNominalSampleRate, &sampleRate) == 0, sampleRate > 0 else { return false }
        guard let inputFormat = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: sampleRate, channels: 2, interleaved: true),
              let outputFormat = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48_000, channels: 2, interleaved: true),
              let converter = AVAudioConverter(from: inputFormat, to: outputFormat) else { return false }
        guard AudioDeviceCreateIOProcIDWithBlock(&io, device, nil, { _, input, _, _, _ in
            let buffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: input))
            guard buffers.reduce(0, { $0 + $1.mNumberChannels }) == clockInputs + 2 else { return }
            guard let buffer = buffers.last, let data = buffer.mData, buffer.mNumberChannels >= 2 else { return }
            let channels = Int(buffer.mNumberChannels)
            let frames = Int(buffer.mDataByteSize) / MemoryLayout<Float>.size / channels
            let samples = data.assumingMemoryBound(to: Float.self)
            // The tap is last, including on virtual output devices with their own inputs.
            var stereo = [Float](repeating: 0, count: frames * 2)
            for frame in 0..<frames {
                stereo[frame * 2] = samples[frame * channels + channels - 2]
                stereo[frame * 2 + 1] = samples[frame * channels + channels - 1]
            }
            if sampleRate == 48_000 {
                stereo.withUnsafeBufferPointer { samples in
                    if let base = samples.baseAddress { output(context, base, UInt32(samples.count)) }
                }
            } else {
                guard let source = AVAudioPCMBuffer(pcmFormat: inputFormat, frameCapacity: AVAudioFrameCount(frames)),
                      let target = AVAudioPCMBuffer(pcmFormat: outputFormat, frameCapacity: AVAudioFrameCount(ceil(Double(frames) * 48_000 / sampleRate) + 32)) else { return }
                source.frameLength = AVAudioFrameCount(frames)
                stereo.withUnsafeBytes { bytes in source.mutableAudioBufferList.pointee.mBuffers.mData!.copyMemory(from: bytes.baseAddress!, byteCount: bytes.count) }
                var provided = false
                var error: NSError?
                let status = converter.convert(to: target, error: &error) { _, state in
                    if provided { state.pointee = .noDataNow; return nil }
                    provided = true; state.pointee = .haveData; return source
                }
                if status != .error, let data = target.audioBufferList.pointee.mBuffers.mData, target.frameLength > 0 {
                    output(context, data.assumingMemoryBound(to: Float.self), UInt32(target.frameLength) * 2)
                }
            }
        }) == 0 else { return false }
        return AudioDeviceStart(device, io) == 0
    }
}

@_cdecl("screenshare_start_app_audio")
func startAppAudio(_ pid: Int32, _ output: @escaping @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<Float>, UInt32) -> Void, _ context: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    guard #available(macOS 14.2, *) else { return nil }
    let capture = AppAudioTap()
    guard capture.start(pid, output, context) else { return nil }
    return Unmanaged.passRetained(capture).toOpaque()
}

@_cdecl("screenshare_stop_app_audio")
func stopAppAudio(_ capture: UnsafeMutableRawPointer) {
    if #available(macOS 14.2, *) { Unmanaged<AppAudioTap>.fromOpaque(capture).release() }
}
