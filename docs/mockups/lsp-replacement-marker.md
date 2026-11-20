# Replacement marker (Yinz VS Code extension)

Approved by Chief (card #569). Renders: `lsp-replacement-marker-0-rest.png` … `-4-action-array.png`.

## User goal
See, without being nagged, where a written form could be stricter (`let` → `const`, `array` → `fixed`), learn why from the hover, and convert in one action.

## Main decision
The only new visual is a **dotted underline on the existing token**, coloured with the extension's own theme colour `yinz.lessonUnderline` (cyan; Dark Modern default `#4FD1E8`, light default `#0E7490`). Everything else is stock VS Code: hover widget, lightbulb, code-action menu.

- Dotted, never a squiggle: squiggles are VS Code's diagnostic style, and this is not a diagnostic. The Problems count stays 0/0.
- Cyan, because most people use dark themes and a blue or dark blue can be too dark. A theme colour, not a hard-coded hex, so users can override it in `workbench.colorCustomizations`. `#4FD1E8` on `#1f1f1f` = 9.12:1 (≥ 3:1 non-text).
- `text-decoration: underline dotted`, 1 px, offset 2 px (`.sugg` in the HTML).

## Interaction
- Hover on the underlined word: three paragraphs, **WHAT:**, **WHAT INSTEAD:**, **WHY:** (bold labels, code in inline code). No signature block, no hover status bar.
- Lightbulb / Ctrl+.: one action per marker under the **Rewrite** group (`refactor.rewrite`, wrench icon).
- Action titles are plain text with single quotes (Chief's choice): `Write 'const' instead of 'let'`, `Write 'fixed' instead of 'array'`. The hover quotes them in the same form.

## Assumptions
- The hover opens below the word (VS Code puts it below when there is no room above), so in panel 1 it covers line 3. Panel 0 shows both markers unobscured.
- Mockup fonts are fallbacks (DejaVu); the real editor uses the user's font settings.
- Dark Modern only, because Chief asked for it; in other themes the marker uses the `yinz.lessonUnderline` default for that theme kind.
