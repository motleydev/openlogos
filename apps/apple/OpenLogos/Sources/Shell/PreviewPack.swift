// B071: the real editions on this Mac. A preview pack (docs/preview-pack.md) is built outside
// the repository and never ships in the app; the app reads it from one place:
//
//   -OPENLOGOS_PREVIEW_PACK <dir>      a launch argument (`none`: run on the placeholder library)
//   OPENLOGOS_PREVIEW_PACK=<dir>       the environment, as `just preview-pack` reads it
//   ~/.cache/openlogos/preview-pack    the default
//
// Each `packs/<id>` is installed with `ReadingHandle.open_pack` over the placeholder library,
// so an edition with no pack (or a pack that does not open) stays a placeholder and every
// chapter of it says so (`ReadingNotice.placeholder`). Text search is the same shape as the
// web's worker: the packs under `search/` are answered off the main thread, by a handle of
// their own (EditionTextSearch), so no keystroke waits for SQLite.
import Foundation

struct PreviewPack {
    let root: URL
    let reading: ReadingHandle
    /// The editions a pack installed, by resource id.
    let installed: [String: ReadingEdition]
    /// What did not open, for the log and the evidence: `wlc: <the core's words>`.
    let problems: [String]
    /// Nil when `search/` is missing or refused: the Text section is then simply empty.
    let search: EditionTextSearch?

    static let environmentKey = "OPENLOGOS_PREVIEW_PACK"
    static let editions = ["bsb", "sblgnt", "wlc"]

    /// Where this launch looks; nil when it was told not to look (`none`), or under XCTest
    /// (a test opens what it needs with `open(root:)`).
    static func location(defaults: UserDefaults = .standard, environment: [String: String] = ProcessInfo.processInfo.environment) -> URL? {
        func url(_ path: String) -> URL? {
            path == "none" ? nil : URL(fileURLWithPath: (path as NSString).expandingTildeInPath, isDirectory: true)
        }
        if let path = defaults.string(forKey: environmentKey) { return url(path) }
        if environment["XCTestConfigurationFilePath"] != nil || environment["XCTestBundlePath"] != nil || NSClassFromString("XCTestCase") != nil {
            return nil
        }
        if let path = environment[environmentKey], !path.isEmpty { return url(path) }
        return FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".cache/openlogos/preview-pack", isDirectory: true)
    }

    /// The pack at `root`, or nil when it holds no edition (a missing directory is the normal
    /// case on a machine without the corpus: the placeholder library, which says so).
    static func open(root: URL) -> PreviewPack? {
        let reading = ReadingHandle.placeholder()
        var installed: [String: ReadingEdition] = [:]
        var problems: [String] = []
        let packs = root.appendingPathComponent("packs", isDirectory: true)
        for id in editions {
            let directory = packs.appendingPathComponent(id, isDirectory: true)
            guard FileManager.default.fileExists(atPath: directory.appendingPathComponent("manifest.json").path) else { continue }
            do {
                installed[id] = try reading.openPack(directory: directory.path)
            } catch {
                problems.append("\(id): \(error)")
            }
        }
        guard !installed.isEmpty else {
            if !problems.isEmpty { NSLog("OpenLogos preview pack at %@: %@", root.path, problems.joined(separator: "; ")) }
            return nil
        }
        var search: EditionTextSearch?
        let directory = root.appendingPathComponent("search", isDirectory: true)
        if FileManager.default.fileExists(atPath: directory.path) {
            do {
                search = try EditionTextSearch(directory: directory.path, titles: installed.mapValues(\.title))
            } catch {
                problems.append("search: \(error)")
            }
        }
        if !problems.isEmpty { NSLog("OpenLogos preview pack at %@: %@", root.path, problems.joined(separator: "; ")) }
        return PreviewPack(root: root, reading: reading, installed: installed, problems: problems, search: search)
    }

    /// The pack this launch reads, if any.
    static func forLaunch() -> PreviewPack? {
        location().flatMap(open(root:))
    }
}

/// The Text section of the palette over the search packs, off the main thread.
///
/// The session's `ShellHandle` holds one lock for everything, so a search on it, even on
/// another thread, would make the main thread wait at its next call. This is a second handle
/// with the provider and a session of its own, used for nothing else; its rows are merged into
/// the session handle's answer as they arrive (`ShellModel.query`). Only the newest request
/// runs: one that a later keystroke has passed is dropped before it reaches SQLite.
final class EditionTextSearch: @unchecked Sendable {
    private let handle: ShellHandle
    private let queue = DispatchQueue(label: "org.openlogos.text-search", qos: .userInteractive)
    private let lock = NSLock()
    private var latest: UInt64 = 0
    private var answer: (generation: UInt64, group: ShellPaletteGroup?)?

    init(directory: String, titles: [String: String]) throws {
        handle = try ShellHandle(sessionJson: ShellSeed.singleColumnJSON)
        handle.setResourceTitles(titles: titles)
        try handle.useEditionSearch(directory: directory)
    }

    /// Asks for the Text section of what is typed. `delivered` is called on the main queue with
    /// this request's generation, unless a later request overtook it. Returns the generation.
    @discardableResult
    func request(text: String, context: ShellPaletteContext, delivered: @escaping @MainActor (UInt64, ShellPaletteGroup?) -> Void) -> UInt64 {
        lock.lock()
        latest += 1
        let generation = latest
        lock.unlock()
        queue.async { [self] in
            guard isLatest(generation) else { return }
            let group = handle.paletteQuery(text: text, context: context).groups.first { $0.section == .text }
            lock.lock()
            let current = latest == generation
            if current { answer = (generation, group) }
            lock.unlock()
            guard current else { return }
            DispatchQueue.main.async {
                MainActor.assumeIsolated { delivered(generation, group) }
            }
        }
        return generation
    }

    /// Cancels what is in flight: nothing more is delivered for it.
    func cancel() {
        lock.lock()
        latest += 1
        answer = nil
        lock.unlock()
    }

    /// Waits for the newest request and returns its answer (Enter on a row must act on the rows
    /// for what is typed now). At most one query's time.
    func settled() -> (generation: UInt64, group: ShellPaletteGroup?)? {
        queue.sync {}
        lock.lock()
        defer { lock.unlock() }
        return answer?.generation == latest ? answer : nil
    }

    private func isLatest(_ generation: UInt64) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return latest == generation
    }
}
