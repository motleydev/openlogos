// B076: the Lexicon tab Word study opens: the word as it was clicked, its language and its
// passage, and an honest line that the entries are not here yet. No glosses are invented.
import SwiftUI

/// What a Lexicon tab was opened for.
struct LexiconEntry: Equatable {
    /// The word as clicked, with its marks.
    let word: String
    /// BCP 47 tag of the text it came from (`grc`, `hbo`).
    let language: String
    /// Its verse (OSIS, `John.3.16`).
    let reference: String
}

struct LexiconView: View {
    @ObservedObject var model: ShellModel
    let tab: ShellTab

    static let placeholder = "Lexicon entries arrive with the lexicon import."

    var body: some View {
        let entry = model.lexiconEntries[tab.id]
        VStack(alignment: .leading, spacing: ShellTokens.Spacing.s) {
            if let entry {
                Text(entry.word)
                    .font(.system(size: model.readingSize + 9, design: .serif))
                    .textSelection(.enabled)
                    .accessibilityIdentifier("shell-lexicon-word")
                Text("\(ShellReference.languageName(entry.language)) · \(ShellReference.human(entry.reference))")
                    .font(ShellTokens.chromeFont)
                    .foregroundStyle(model.theme.chromeColor)
                    .accessibilityIdentifier("shell-lexicon-source")
            } else {
                Text("Lexicon")
                    .font(.system(size: model.readingSize + 4, weight: .semibold, design: .serif))
                Text("Choose Word study (W) in a Greek or Hebrew column and click a word.")
                    .font(ShellTokens.chromeFont)
                    .foregroundStyle(model.theme.chromeColor)
            }
            Text(Self.placeholder)
                .font(ShellTokens.chromeFont)
                .foregroundStyle(model.theme.chromeColor)
                .padding(.top, ShellTokens.Spacing.s)
                .accessibilityIdentifier("shell-lexicon-placeholder")
        }
        .padding(ShellTokens.Spacing.xl)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Lexicon")
        .accessibilityIdentifier("shell-lexicon")
    }
}
