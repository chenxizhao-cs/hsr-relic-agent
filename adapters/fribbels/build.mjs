import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import { copyFileSync } from 'node:fs'
import { build } from '../../upstream/hsr-optimizer/node_modules/vite/dist/node/index.js'

const here = fileURLToPath(new URL('.', import.meta.url))
const upstream = resolve(here, '../../upstream/hsr-optimizer')
const commit = 'df630a0488a64eeb740e4e0c14f265d96b9f6f8f'
const git = (...args) => execFileSync('git', ['-C', upstream, ...args], { encoding: 'utf8' }).trim()
if (git('rev-parse', 'HEAD') !== commit || git('status', '--porcelain', '--untracked-files=all')) {
  throw new Error(`Adapter requires clean upstream ${commit}; no source changes are applied automatically.`)
}
await build({
  configFile: false,
  root: here,
  publicDir: false,
  define: { __UPSTREAM_COMMIT__: JSON.stringify(commit) },
  resolve: { alias: [
    { find: 'cross-fetch', replacement: resolve(upstream, 'src/lib/utils/nativeFetch.ts') },
    ...['lib', 'types', 'data'].map(name => ({
      find: new RegExp(`^${name}/(.*)$`), replacement: `${resolve(upstream, 'src', name)}/$1`,
    })),
  ] },
  ssr: { noExternal: true },
  build: {
    ssr: resolve(here, 'adapter.ts'), outDir: resolve(here, 'dist'), emptyOutDir: true,
    target: 'node22', minify: false, sourcemap: false,
    rollupOptions: { output: { entryFileNames: 'adapter.mjs' } },
  },
})
copyFileSync(resolve(upstream, 'LICENSE.md'), resolve(here, 'dist/FRIBBELS-LICENSE.md'))
