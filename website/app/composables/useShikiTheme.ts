// Client-side highlight helper — SSR uses the shiki.server.ts plugin directly
export const useShikiHighlight = async (code: string, lang = 'yinz'): Promise<string> => {
  const { createHighlighter } = await import('shiki')
  const [grammar, theme] = await Promise.all([
    import('~/assets/grammars/ynz.tmLanguage.json'),
    import('~/assets/themes/yinz-coal.json'),
  ])
  // JSON-import literal types are narrower than Shiki's registration types — valid TextMate JSON at runtime
  const hl = await createHighlighter({
    langs: [grammar.default as unknown as import('shiki').LanguageRegistration],
    themes: [theme.default as unknown as import('shiki').ThemeRegistration],
  })
  return hl.codeToHtml(code, { lang, theme: 'yinz-coal' })
}
