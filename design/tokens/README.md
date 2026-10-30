# design/tokens

Single-source design tokens generated per platform (docs/adr/0001-rust-core-native-ui.md,
specs/16-design-guide.md).

- `shell.json` (B072): the workspace shell's fonts, reading sizes and line spacings, the three
  themes, highlight swatches, spacing, radii and chrome metrics. The web shell carries them as
  CSS custom properties in `apps/web/shell/shell.css` and `just test-shell-web` fails when the
  two disagree; the Mac app mirrors them as a Swift enum. Shortcuts are not tokens: hosts read
  them from the core command registry.
