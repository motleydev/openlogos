// B072: the shell's design tokens, the same values the web shell reads from
// design/tokens/shell.json (written by the web host; this enum mirrors it by hand, and
// ShellTests checks the values). Reading text in the system serif (New York), chrome in the
// system UI font, three themes, four highlight swatches, spacing and radii.
import SwiftUI

enum ShellTokens {
    // MARK: Type

    /// Reading size steps in points; index 2 (19) is the default.
    static let readingSizes: [CGFloat] = [15, 17, 19, 21, 24, 28, 32]
    static let defaultReadingSizeIndex = 2
    /// Line-height multiples; index 1 (1.5) is the default.
    static let lineSpacings: [CGFloat] = [1.35, 1.5, 1.7]
    static let defaultLineSpacingIndex = 1
    static let chromeSize: CGFloat = 13
    static let tabLabelSize: CGFloat = 12
    static let badgeSize: CGFloat = 11

    static var chromeFont: Font { .system(size: chromeSize) }
    static var tabFont: Font { .system(size: tabLabelSize) }
    static var badgeFont: Font { .system(size: badgeSize, weight: .semibold).monospacedDigit() }

    /// Column width above which the text is centred at the measure: the core's
    /// `MEASURE_CAP_STANDARD_PT` (crates/workspaces/src/lib.rs), which the bindings do not
    /// expose yet.
    static let measureCap: CGFloat = 640

    // MARK: Geometry

    static let paletteHeight: CGFloat = 44
    static let tabStripHeight: CGFloat = 28
    static let identityRowHeight: CGFloat = 32
    static let badgeSide: CGFloat = 20
    static let toolWellWidth: CGFloat = 40
    static let toolButtonSide: CGFloat = 32
    static let toolWellInset: CGFloat = 12
    static let accentUnderline: CGFloat = 2
    static let rule: CGFloat = 1

    enum Spacing {
        static let xs: CGFloat = 4
        static let s: CGFloat = 8
        static let m: CGFloat = 12
        static let l: CGFloat = 16
        static let xl: CGFloat = 24
    }

    enum Radius {
        static let control: CGFloat = 6
        static let toolWell: CGFloat = 10
    }

    // MARK: Colour

    struct Theme: Equatable, Identifiable {
        let id: String
        let title: String
        let page: UInt32
        let text: UInt32
        let chrome: UInt32
        let rules: UInt32
        let accent: UInt32
        let isDark: Bool

        var pageColor: Color { Color(hex: page) }
        var textColor: Color { Color(hex: text) }
        var chromeColor: Color { Color(hex: chrome) }
        var rulesColor: Color { Color(hex: rules) }
        var accentColor: Color { Color(hex: accent) }
        /// The tool well and the palette's result list: slightly off the page colour.
        var wellColor: Color { isDark ? Color(hex: page).mix(with: .white, by: 0.06) : Color(hex: page).mix(with: .black, by: 0.035) }
    }

    static let light = Theme(id: "light", title: "Light", page: 0xFBFAF7, text: 0x1C1B19, chrome: 0x5F5B55, rules: 0xE4E1DA, accent: 0x2F5BD3, isDark: false)
    static let sepia = Theme(id: "sepia", title: "Sepia", page: 0xF4ECDD, text: 0x2B2620, chrome: 0x6B6152, rules: 0xE0D5C1, accent: 0x8A4B16, isDark: false)
    static let dark = Theme(id: "dark", title: "Dark", page: 0x161616, text: 0xE9E6E1, chrome: 0xA39E96, rules: 0x2C2B29, accent: 0x7EA2FF, isDark: true)
    static let themes = [light, sepia, dark]

    static func theme(named name: String) -> Theme? { themes.first { $0.id == name.lowercased() } }

    struct Swatch: Equatable, Identifiable {
        let id: String
        let title: String
        let rgb: UInt32
        let alpha: Double
        var color: Color { Color(hex: rgb).opacity(alpha) }
        /// The solid colour, for the swatch well and for ink.
        var solid: Color { Color(hex: rgb) }
        var platformColor: PlatformColor { PlatformColor(hex: rgb, alpha: alpha) }
    }

    static let swatches = [
        Swatch(id: "yellow", title: "Yellow", rgb: 0xF5D90A, alpha: 0.35),
        Swatch(id: "green", title: "Green", rgb: 0x4CC38A, alpha: 0.30),
        Swatch(id: "blue", title: "Blue", rgb: 0x5B9BFF, alpha: 0.30),
        Swatch(id: "pink", title: "Pink", rgb: 0xF25C9A, alpha: 0.28),
    ]

    /// Ink widths the width dot cycles through, in points.
    static let inkWidths: [Double] = [2, 4, 6]
}

extension Color {
    init(hex: UInt32) {
        self.init(.sRGB,
                  red: Double((hex >> 16) & 0xFF) / 255,
                  green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255,
                  opacity: 1)
    }
}

extension PlatformColor {
    convenience init(hex: UInt32, alpha: Double = 1) {
        let r = CGFloat((hex >> 16) & 0xFF) / 255, g = CGFloat((hex >> 8) & 0xFF) / 255, b = CGFloat(hex & 0xFF) / 255
        #if canImport(UIKit)
        self.init(red: r, green: g, blue: b, alpha: CGFloat(alpha))
        #else
        self.init(srgbRed: r, green: g, blue: b, alpha: CGFloat(alpha))
        #endif
    }
}
