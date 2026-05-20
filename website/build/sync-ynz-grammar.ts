import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(__dirname, '../..')

const src = resolve(repoRoot, 'tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json')
const dst = resolve(__dirname, '../app/assets/grammars/ynz.tmLanguage.json')

try {
  const srcContent = readFileSync(src, 'utf8')
  const srcHash = createHash('sha256').update(srcContent).digest('hex')

  writeFileSync(dst, srcContent, 'utf8')

  const dstContent = readFileSync(dst, 'utf8')
  const dstHash = createHash('sha256').update(dstContent).digest('hex')

  if (srcHash !== dstHash) {
    console.error(`ERROR: sha256 mismatch after write — src: ${srcHash} dst: ${dstHash}`)
    process.exit(1)
  }

  console.log(`sync-ynz-grammar: ok (sha256: ${srcHash.slice(0, 12)}...)`)
} catch (err: unknown) {
  if (err instanceof Error && 'code' in err && (err as NodeJS.ErrnoException).code === 'ENOENT') {
    console.error(`ERROR: source grammar not found at ${src}`)
    console.error('  Run from the repo root — the grammar lives at tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json')
    process.exit(1)
  }
  throw err
}
