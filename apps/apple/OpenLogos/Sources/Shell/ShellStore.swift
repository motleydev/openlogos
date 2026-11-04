// B074: the user store on the Mac (specs/24). The core holds everything a reader would miss
// after a quit (workspaces, columns, tabs and their scroll anchors, the tool, the notes, the
// highlights, the ink) and writes it as one JSON document (`exportStore`). This host only
// reads and writes the bytes: `UserStoreFile` is the file in Application Support with its
// backups, written atomically on a serial queue; `ShellPersistence` restores it at launch,
// saves 1 s after the last change (`storeConstants().saveDebounceMs`) and at once on quit or
// when the window closes, and tells the reader once when a file could not be read whole.
//
// The real store is never touched by tests or by scripted runs: XCTest, a snapshot, a
// measurement or a prototype view get no store unless `-OPENLOGOS_STORE <directory>` names one.
import Foundation
import QuartzCore
#if canImport(AppKit)
import AppKit
#endif

/// The store file and its backups in one directory.
final class UserStoreFile: @unchecked Sendable {
    let directory: URL
    let url: URL
    private var temporary: URL { directory.appendingPathComponent("user-store.json.tmp") }
    private let queue = DispatchQueue(label: "org.openlogos.user-store", qos: .utility)
    static let maxBackups = 5

    init(directory: URL) {
        self.directory = directory
        url = directory.appendingPathComponent("user-store.json")
    }

    /// `~/Library/Application Support/OpenLogos` (inside the container when sandboxed).
    static var applicationSupport: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library/Application Support")
        return base.appendingPathComponent("OpenLogos", isDirectory: true)
    }

    /// The saved bytes, or nil when there is no file.
    func read() -> Data? {
        try? Data(contentsOf: url)
    }

    /// Writes on the serial queue; `done` runs on the main queue with whether it worked and
    /// how long the write took (ms).
    func write(_ json: String, done: @escaping @MainActor (Bool, Double) -> Void) {
        queue.async { [self] in
            let t0 = CACurrentMediaTime()
            let ok = writeFile(Data(json.utf8))
            let ms = (CACurrentMediaTime() - t0) * 1000
            DispatchQueue.main.async { MainActor.assumeIsolated { done(ok, ms) } }
        }
    }

    /// Writes now, after any write already queued (the quit path).
    @discardableResult
    func writeNow(_ json: String) -> Bool {
        queue.sync { writeFile(Data(json.utf8)) }
    }

    /// Waits for the queued writes.
    func flush() { queue.sync {} }

    /// `user-store.json.tmp`, then swapped in: a crash mid-write leaves the previous file.
    private func writeFile(_ data: Data) -> Bool {
        let manager = FileManager.default
        do {
            try manager.createDirectory(at: directory, withIntermediateDirectories: true)
            try data.write(to: temporary)
            if manager.fileExists(atPath: url.path) {
                _ = try manager.replaceItemAt(url, withItemAt: temporary)
            } else {
                try manager.moveItem(at: temporary, to: url)
            }
            return true
        } catch {
            try? manager.removeItem(at: temporary)
            return false
        }
    }

    /// Keeps a copy of bytes that could not be restored whole, as
    /// `user-store.backup-<ISO 8601>.json` (`:` as `-`), and only the newest five.
    @discardableResult
    func backup(_ data: Data, at date: Date = Date()) -> URL? {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let stamp = formatter.string(from: date).replacingOccurrences(of: ":", with: "-")
        let target = directory.appendingPathComponent("user-store.backup-\(stamp).json")
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try data.write(to: target, options: .atomic)
        } catch {
            return nil
        }
        for old in backups().dropFirst(Self.maxBackups) { try? FileManager.default.removeItem(at: old) }
        return target
    }

    /// The backups, newest first (the stamps sort as times).
    func backups() -> [URL] {
        let names = (try? FileManager.default.contentsOfDirectory(atPath: directory.path)) ?? []
        return names.filter { $0.hasPrefix("user-store.backup-") && $0.hasSuffix(".json") }.sorted(by: >)
            .map { directory.appendingPathComponent($0) }
    }
}

/// What a launch restored, and how long its parts took.
struct StoreRestore {
    /// The report, when the file was read; nil when there was none or it was refused.
    var report: StoreRestoreReport?
    /// The refusal's kind (`not_json`, `not_a_store`, `unsupported_version`, `empty`).
    var refused: String?
    /// The words shown once, when something could not be restored.
    var notice: String?
    var backup: URL?
    var readMs: Double = 0
    var importMs: Double = 0
    var bytes = 0
    /// When the file read began (CACurrentMediaTime).
    var started: CFTimeInterval = 0
    /// The file as read, when it restored without issues (a save of the same bytes is skipped).
    var cleanText: String?
}

/// The save policy over a shell's store file (specs/24 §4).
@MainActor
final class ShellPersistence {
    let file: UserStoreFile
    let handle: ShellHandle
    /// Runs before a save, so the core has what an editor still holds (a note's last keys).
    var beforeSave: () -> Void = {}
    /// Said once when writes keep failing.
    var onFailure: (String) -> Void = { _ in }
    /// The change count at the last save (or at the restore).
    private(set) var lastSaved: UInt64
    private(set) var failures = 0
    private(set) var saves = 0
    /// How long each `exportStore` took on the main thread, and each write off it (ms).
    private(set) var exportSamples: [Double] = []
    private(set) var writeSamples: [Double] = []
    /// What the file holds now: a save that would write the same bytes writes nothing.
    private var written: String?
    private var timer: Timer?
    private var observers: [NSObjectProtocol] = []
    let debounce: TimeInterval

    static let failureNotice = "Your work could not be saved; OpenLogos will keep trying."
    static let failuresBeforeNotice = 3

    /// `lastSaved`: the count before the restore (a restore that set something aside moved
    /// it, so the repaired store is saved).
    init(file: UserStoreFile, handle: ShellHandle, lastSaved: UInt64? = nil, written: String? = nil) {
        self.file = file
        self.handle = handle
        self.written = written
        self.lastSaved = lastSaved ?? handle.changeCount()
        debounce = Double(storeConstants().saveDebounceMs) / 1000
        #if canImport(AppKit)
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: NSApplication.willTerminateNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.saveNow() }
        })
        observers.append(center.addObserver(forName: NSWindow.willCloseNotification, object: nil, queue: .main) { [weak self] note in
            MainActor.assumeIsolated {
                guard let window = note.object as? NSWindow, window.level == .normal, !(window is NSPanel) else { return }
                self?.saveNow()
            }
        })
        #endif
    }

    deinit {
        observers.forEach(NotificationCenter.default.removeObserver)
    }

    /// Whether the core has changes the file does not.
    var isDirty: Bool { handle.changeCount() != lastSaved }

    /// After anything that may have changed the store: (re)starts the debounce when the count
    /// moved since the last save.
    func changed() {
        guard isDirty else { return }
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: debounce, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated { self?.save() }
        }
    }

    /// The debounced save: the export here, on the main thread (under a millisecond at the
    /// acceptance size), the write on the file's queue.
    func save() {
        timer?.invalidate()
        timer = nil
        beforeSave()
        guard isDirty else { return }
        let count = handle.changeCount()
        let t0 = CACurrentMediaTime()
        let json = handle.exportStore()
        record(&exportSamples, (CACurrentMediaTime() - t0) * 1000)
        if json == written { return finished(true, count: count) }
        written = json
        file.write(json) { [weak self] ok, ms in
            guard let self else { return }
            self.record(&self.writeSamples, ms)
            self.finished(ok, count: count)
        }
    }

    /// The quit and window-close path: written before returning.
    func saveNow() {
        timer?.invalidate()
        timer = nil
        beforeSave()
        file.flush()
        guard isDirty else { return }
        let count = handle.changeCount()
        let json = handle.exportStore()
        if json == written { return finished(true, count: count) }
        written = json
        finished(file.writeNow(json), count: count)
    }

    private func finished(_ ok: Bool, count: UInt64) {
        if ok {
            saves += 1
            failures = 0
            lastSaved = max(lastSaved, count)
            // A change made while the write was queued is saved by its own debounce.
            changed()
        } else {
            // The old file stays; the next change tries again.
            written = nil
            failures += 1
            if failures == Self.failuresBeforeNotice { onFailure(Self.failureNotice) }
        }
    }

    private func record(_ samples: inout [Double], _ value: Double) {
        if samples.count < 4096 { samples.append(value) }
    }

    // MARK: Restore (specs/24 §5, §6)

    /// Reads the file and imports it into the shell, before anything is rendered: a refused
    /// file leaves the seed session and is kept as a backup; a store with parts set aside is
    /// restored and kept as a backup too. Either way the reader is told once.
    static func restore(_ handle: ShellHandle, from file: UserStoreFile) -> StoreRestore {
        var result = StoreRestore()
        let t0 = CACurrentMediaTime()
        result.started = t0
        guard let data = file.read() else { return result }
        result.readMs = (CACurrentMediaTime() - t0) * 1000
        result.bytes = data.count
        let t1 = CACurrentMediaTime()
        do {
            let text = String(decoding: data, as: UTF8.self)
            let report = try handle.importStore(json: text)
            result.report = report
            if report.issues.isEmpty { result.cleanText = text }
            if !report.issues.isEmpty {
                result.backup = file.backup(data)
                result.notice = notice(for: report.issues)
            }
        } catch StoreCallError.Refused(let kind, _) {
            result.refused = kind
            result.backup = file.backup(data)
            result.notice = notice(refused: kind)
        } catch {
            result.refused = "unknown"
            result.backup = file.backup(data)
            result.notice = notice(refused: "unknown")
        }
        result.importMs = (CACurrentMediaTime() - t1) * 1000
        return result
    }

    /// The words for a file refused whole.
    static func notice(refused kind: String) -> String {
        kind == "unsupported_version"
            ? "Your saved workspace was written by a newer version of OpenLogos; a copy was kept."
            : "Your saved workspace could not be read; a copy was kept."
    }

    /// `Your workspaces were reset; 2 highlights could not be restored`: what was set aside,
    /// session first, then notes, highlights and ink.
    static func notice(for issues: [StoreIssue]) -> String? {
        var parts: [String] = []
        if issues.contains(where: { $0.part == "session" }) { parts.append("Your workspaces were reset") }
        for (part, one, many) in [("note", "note", "notes"), ("highlight", "highlight", "highlights"), ("ink", "ink stroke", "ink strokes")] {
            let n = issues.filter { $0.part == part }.count
            if n > 0 { parts.append("\(n) \(n == 1 ? one : many) could not be restored") }
        }
        return parts.isEmpty ? nil : parts.joined(separator: "; ")
    }

    // MARK: File read to first painted text (specs/24 §5)

    #if os(macOS)
    /// Armed at a restore: the first stream of that workspace to lay its text out answers.
    static var paintProbe: (started: CFTimeInterval, workspace: String)?
    /// File read → text laid out in the view, and → the next display frame (ms).
    static var firstPaint: (laidOut: Double, frame: Double)?
    private static var frameLink: CADisplayLink?
    private static var frameDone: ((CFTimeInterval) -> Void)?

    static func firstText(_ stream: ReadingStream) {
        guard let probe = paintProbe, stream.workspaceID == probe.workspace, let view = stream.container,
              (stream.surface.storage?.length ?? 0) > 0 else { return }
        paintProbe = nil
        let laidOut = CACurrentMediaTime()
        frameDone = { frame in firstPaint = ((laidOut - probe.started) * 1000, (frame - probe.started) * 1000) }
        frameLink = view.displayLink(target: FrameTarget.shared, selector: #selector(FrameTarget.tick(_:)))
        frameLink?.add(to: .main, forMode: .common)
    }

    /// The display link's target: the first frame after the text was laid out.
    final class FrameTarget: NSObject {
        static let shared = FrameTarget()
        @objc func tick(_ link: CADisplayLink) {
            let now = CACurrentMediaTime()
            link.invalidate()
            MainActor.assumeIsolated {
                ShellPersistence.frameLink = nil
                ShellPersistence.frameDone?(now)
                ShellPersistence.frameDone = nil
            }
        }
    }
    #endif

    // MARK: Which file

    /// The store a launch uses: `-OPENLOGOS_STORE <directory>` (`none`: no store), else none
    /// under XCTest or a scripted run (a snapshot, a measurement, a prototype view), else
    /// Application Support.
    static func launchFile(defaults: UserDefaults = .standard, environment: [String: String] = ProcessInfo.processInfo.environment) -> UserStoreFile? {
        if let path = defaults.string(forKey: "OPENLOGOS_STORE") {
            return path == "none" ? nil : UserStoreFile(directory: URL(fileURLWithPath: (path as NSString).expandingTildeInPath, isDirectory: true))
        }
        if environment["XCTestConfigurationFilePath"] != nil || environment["XCTestBundlePath"] != nil || NSClassFromString("XCTestCase") != nil {
            return nil
        }
        let scripted = ["OPENLOGOS_SNAPSHOT", "OPENLOGOS_SHELL_MEASURE", "OPENLOGOS_VIEW", "OPENLOGOS_STORE_MEASURE", "OPENLOGOS_RESTORE_MEASURE",
                        "OPENLOGOS_BUILD_STORE", "OPENLOGOS_STORE_DEMO", "OPENLOGOS_DRAG"]
        for key in scripted where defaults.object(forKey: key) != nil {
            return nil
        }
        return UserStoreFile(directory: UserStoreFile.applicationSupport)
    }
}
