# Replacement marker (Yinz VS Code extension)

Approved by Chief (card #569). Renders: `lsp-replacement-marker-0-rest.png` … `-4-action-array.png`.

## User goal
See, without being nagged, where a written form could be stricter (`let` → `const`, `array` → `fixed`), learn why from the hover, and convert in one action.

## Main decision
The only new visual is a **dotted cyan underline on the existing token**, in a colour the extension contributes: `yinz.lessonUnderline`. Everything else is stock VS Code: hover widget, lightbulb, code-action menu.

| Theme kind | Default | Contrast on editor background |
|---|---|---|
| Dark | `#4FD1E8` | 9.12:1 on `#1f1f1f` (Dark Modern) |
| Light | `#0E7490` | 5.36:1 on `#ffffff` (Light Modern) |
| High contrast | `#4FD1E8` (dark), `#0E7490` (light) | 11.62:1 on `#000000` (dark), 5.36:1 on `#ffffff` (light) |

- Users override it with `workbench.colorCustomizations` (`"yinz.lessonUnderline": "#…"`).
- Cyan, not the theme's info blue: most users run dark themes, and a blue or dark blue underline reads too dark there (Chief's call).
- Dotted, never a squiggle: squiggles are VS Code's diagnostic style, and this is not a diagnostic. The Problems count stays 0/0.
- All defaults clear the 3:1 non-text contrast minimum (WCAG 2.2 AA).
- `text-decoration: underline dotted`, 1 px, offset 2 px (`.sugg` in the HTML).

## Interaction
- Hover on the underlined word: three paragraphs, **WHAT:**, **WHAT INSTEAD:**, **WHY:** (bold labels, code in inline code). No signature block, no hover status bar.
- Lightbulb / Ctrl+.: one action per marker under the **Rewrite** group (`refactor.rewrite`, wrench icon).
- Action titles are plain text with single quotes (Chief's choice): `Write 'const' instead of 'let'`, `Write 'Fixed' instead of 'Array'`. The hover quotes them in the same form.

## Assumptions
- The hover opens below the word (VS Code puts it below when there is no room above), so in panel 1 it covers line 3. Panel 0 shows both markers unobscured.
- Mockup fonts are fallbacks (DejaVu); the real editor uses the user's font settings.
- Dark Modern only, because Chief asked for it; in other themes the marker uses the `yinz.lessonUnderline` default for that theme kind.
