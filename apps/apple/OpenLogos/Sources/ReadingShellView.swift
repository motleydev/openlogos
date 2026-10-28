// The shell's one screen: a reading surface, not a form. Monochrome, a bundled system serif
// (New York through `.design(.serif)`; no font download), a measure of about 65 Latin
// characters, generous leading, Hebrew right-to-left with its points and cantillation intact,
// and Dynamic Type throughout. At the foot, a small reference field that calls the core's
// parseReference and shows the OSIS and human forms, and the core API version.
//
// B063 shell. B062 replaces this with the TextKit 2 reader (specs/16-design-guide.md sets the
// visual baseline this follows loosely: serif, 1.45–1.65 leading, 55–75 character measure).
import SwiftUI

struct ReadingShellView: View {
    @State private var reference = "Jn 3:16, 18; Rom 8:28-30"
    private let passages = FixtureTexts.passages()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 44) {
                ForEach(passages) { passage in
                    PassageView(passage: passage)
                }
                ReferenceField(reference: $reference)
                Text("OpenLogos shell · core API \(apiVersion())")
                    .font(.system(.footnote, design: .serif))
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: 620, alignment: .leading)
            .padding(.horizontal, 28)
            .padding(.vertical, 56)
            .frame(maxWidth: .infinity)
        }
        .textSelection(.enabled)
    }
}

/// A heading, the verses as one paragraph with subordinate verse numbers, and the attribution.
private struct PassageView: View {
    let passage: Passage
    // Leading scales with the text so large type keeps its air (about 1.5 line height).
    @ScaledMetric(relativeTo: .title3) private var leading: CGFloat = 9

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            Text(passage.heading)
                .font(.system(.title, design: .serif))
                .accessibilityAddTraits(.isHeader)
            paragraph
                .font(.system(.title3, design: .serif))
                .lineSpacing(leading)
            Text(passage.attribution)
                .font(.system(.caption, design: .serif))
                .foregroundStyle(.secondary)
        }
        .multilineTextAlignment(.leading)
        .frame(maxWidth: .infinity, alignment: .leading)
        // Hebrew lays out right-to-left as a whole: alignment, verse-number side and wrapping.
        .environment(\.layoutDirection, passage.isRightToLeft ? .rightToLeft : .leftToRight)
    }

    private var paragraph: Text {
        passage.verses.enumerated().reduce(Text("")) { paragraph, element in
            let (index, verse) = element
            let separator = index == 0 ? Text("") : Text(" ")
            // Marks ride on the base letters as the core stores them: no normalisation here.
            let body = Text(verse.text)
            guard !verse.number.isEmpty else { return paragraph + separator + body }
            let number = Text(verse.number)
                .font(.system(.caption2, design: .serif))
                .foregroundStyle(.secondary)
                .baselineOffset(7)
            return paragraph + separator + number + Text("\u{2009}") + body
        }
    }
}

/// The small field: type a citation, see what the core makes of it.
private struct ReferenceField: View {
    @Binding var reference: String

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            TextField("Reference", text: $reference)
                .font(.system(.body, design: .serif))
                .textFieldStyle(.plain)
                .autocorrectionDisabled()
                #if os(iOS)
                .textInputAutocapitalization(.never)
                #endif
                .accessibilityLabel("Reference")
            Divider()
            result
                .font(.system(.footnote, design: .monospaced))
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
        }
    }

    @ViewBuilder private var result: some View {
        switch Result(catching: { try parseReference(text: reference) }) {
        case .success(let set):
            VStack(alignment: .leading, spacing: 4) {
                Text("OSIS  \(set.osis)")
                Text("Human \(set.human)")
                if !set.flags.isEmpty {
                    Text("Flags \(set.flags.map { String(describing: $0) }.joined(separator: ", "))")
                }
            }
        case .failure(let error):
            Text("Not a reference: \(String(describing: error))")
        }
    }
}

#Preview {
    ReadingShellView()
}
