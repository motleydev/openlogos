// B072: the keyboard, read from the core registry (specs/22 §6). Every shortcut string a
// command lists for the Mac (`Cmd+K`, `Alt+Shift+{n}`, `Ctrl+Tab`, `/`, `V`) becomes a chord
// pattern; a key event becomes a chord and is looked up. Nothing is bound here that the
// registry does not list. Keys are named by their position on the US layout (key codes), so
// ⌥ and ⇧ never change which key was meant.
import Foundation
#if canImport(AppKit)
import AppKit
#endif

/// A key with its modifiers. `key` is upper case for letters, the character for digits and
/// punctuation, or a name (`Tab`).
struct KeyChord: Hashable, CustomStringConvertible {
    var key: String
    var command = false
    var option = false
    var shift = false
    var control = false

    /// No ⌘, ⌥ or ⌃: a bare key, live only when no text field has focus.
    var isBare: Bool { !command && !option && !control }

    var description: String {
        (control ? "⌃" : "") + (option ? "⌥" : "") + (shift ? "⇧" : "") + (command ? "⌘" : "") + key
    }

    /// Parses a registry shortcut (`Alt+Shift+]`); `{n}` stays as the key.
    init?(registry text: String) {
        var parts = text.split(separator: "+", omittingEmptySubsequences: false).map(String.init)
        // `Cmd++` would split into an empty last part; the registry has none, but be safe.
        if text.hasSuffix("++") { parts = Array(parts.dropLast(2)) + ["+"] }
        guard let last = parts.popLast(), !last.isEmpty else { return nil }
        key = last.count == 1 ? last.uppercased() : last
        for modifier in parts {
            switch modifier {
            case "Cmd": command = true
            case "Alt": option = true
            case "Shift": shift = true
            case "Ctrl": control = true
            default: return nil
            }
        }
    }

    init(key: String, command: Bool = false, option: Bool = false, shift: Bool = false, control: Bool = false) {
        self.key = key
        self.command = command
        self.option = option
        self.shift = shift
        self.control = control
    }

    #if canImport(AppKit)
    /// The chord of a key-down event, or nil for a key the registry never names.
    init?(event: NSEvent) {
        guard let key = KeyChord.keyName(code: event.keyCode) else { return nil }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        self.init(key: key, command: flags.contains(.command), option: flags.contains(.option),
                  shift: flags.contains(.shift), control: flags.contains(.control))
    }
    #endif

    /// ANSI key codes to key names.
    static func keyName(code: UInt16) -> String? {
        let table: [UInt16: String] = [
            0: "A", 11: "B", 8: "C", 2: "D", 14: "E", 3: "F", 5: "G", 4: "H", 34: "I", 38: "J", 40: "K", 37: "L",
            46: "M", 45: "N", 31: "O", 35: "P", 12: "Q", 15: "R", 1: "S", 17: "T", 32: "U", 9: "V", 13: "W", 7: "X",
            16: "Y", 6: "Z",
            18: "1", 19: "2", 20: "3", 21: "4", 23: "5", 22: "6", 26: "7", 28: "8", 25: "9", 29: "0",
            33: "[", 30: "]", 44: "/", 48: "Tab",
        ]
        return table[code]
    }
}

/// What a chord runs.
struct KeyBinding: Equatable {
    let commandID: String
    let column: UInt32?
    /// The registry marks the command a bare-key command, or the chord has no modifier.
    let bare: Bool
}

struct ShellKeymap {
    private var exact: [KeyChord: KeyBinding] = [:]
    /// `{n}` families: the chord with `key == "{n}"`.
    private var numbered: [KeyChord: String] = [:]

    init(registry: [ShellCommandInfo], platform: ShellPlatform = .mac) {
        for command in registry {
            let shortcuts: [String]
            switch platform {
            case .web: shortcuts = command.shortcuts.web
            case .mac: shortcuts = command.shortcuts.mac
            case .ipad: shortcuts = command.shortcuts.ipad
            }
            for text in shortcuts {
                guard let chord = KeyChord(registry: text) else { continue }
                if chord.key == "{n}" {
                    numbered[chord] = command.id
                } else {
                    exact[chord] = KeyBinding(commandID: command.id, column: nil, bare: command.bareKey || chord.isBare)
                }
            }
        }
    }

    func binding(for chord: KeyChord) -> KeyBinding? {
        if let binding = exact[chord] { return binding }
        if let n = UInt32(chord.key), (1...9).contains(n) {
            var pattern = chord
            pattern.key = "{n}"
            if let id = numbered[pattern] { return KeyBinding(commandID: id, column: n, bare: false) }
        }
        return nil
    }

    /// Every chord bound, for the tests.
    var boundCount: Int { exact.count + numbered.count }
}
