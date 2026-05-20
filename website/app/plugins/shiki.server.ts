// Server-only Shiki plugin using JS regex engine (no WASM — Nitro compatible)
import type { LanguageRegistration, ThemeRegistration } from 'shiki'
import { createHighlighter } from 'shiki'
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript'
// JSON-import literal types are narrower than Shiki's registration types — both are valid TextMate JSON at runtime
import grammarJson from '~/assets/grammars/ynz.tmLanguage.json'
import themeJson from '~/assets/themes/yinz-coal.json'

// Shiki v4 registers by grammar `name` field — "Yinz" per the tmLanguage.json
const LANG_ID = grammarJson.name

export default defineNuxtPlugin(async () => {
  const hl = await createHighlighter({
    langs: [grammarJson as unknown as LanguageRegistration],
    themes: [themeJson as unknown as ThemeRegistration],
    engine: createJavaScriptRegexEngine(),
  })

  return {
    provide: {
      // Single-language M1 highlight — no lang param; multi-lang support deferred to M5+
      shikiHighlight: (code: string): string =>
        hl.codeToHtml(code, { lang: LANG_ID, theme: 'yinz-coal' }),
    },
  }
})
