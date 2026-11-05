// B076: the Lexicon tab Word study opens (`tab.open_lexicon`): the word as it was clicked
// (the core's `lexiconWord`), its language and its passage, and an honest line that the
// entries are not here yet. No glosses are invented. B074: the language is the one the core
// recorded from the clicked edition (`lexiconLanguage`), so a restored tab looks up in the
// right lexicon.
import SwiftUI

struct LexiconView: View {
    @ObservedObject var model: ShellModel
    let tab: ShellTab

    static let placeholder = "Lexicon entries arrive with the lexicon import."

    /// The lexicon the word is looked up in: the clicked edition's language, else the script's.
    static func language(_ tab: ShellTab, word: String) -> String {
        tab.lexiconLanguage ?? ShellReference.language(ofWord: word)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: ShellTokens.Spacing.s) {
            if let word = tab.lexiconWord {
                Text(word)
                    .font(.system(size: model.readingSize * 1.5, weight: .semibold, design: .serif))
                    .textSelection(.enabled)
                    .accessibilityIdentifier("shell-lexicon-word")
                Text("\(ShellReference.languageName(Self.language(tab, word: word))) · \(ShellReference.human(tab.reference))")
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
