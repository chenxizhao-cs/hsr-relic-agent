import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { build } from '../upstream/hsr-optimizer/node_modules/vite/dist/node/index.js'

const root = fileURLToPath(new URL('..', import.meta.url))
const upstream = resolve(root, 'upstream/hsr-optimizer')
execFileSync(process.execPath, [resolve(root, 'adapters/recommendations/prepare.mjs')], { stdio: 'inherit' })
// Reuse the existing pinned-version check and evaluator build.
execFileSync(process.execPath, [resolve(root, 'adapters/fribbels/build.mjs')], { stdio: 'inherit' })
await build({
  configFile: false,
  root,
  publicDir: false,
  resolve: {
    alias: ['lib', 'types', 'data'].map((name) => ({
      find: new RegExp(`^${name}/(.*)$`),
      replacement: `${resolve(upstream, 'src', name)}/$1`,
    })),
  },
  ssr: { noExternal: true },
  build: {
    ssr: resolve(root, 'web/asset-manifest.ts'),
    outDir: resolve(root, 'web/.generated'),
    emptyOutDir: true,
    target: 'node22',
    minify: false,
    rollupOptions: { output: { entryFileNames: 'asset-manifest.mjs' } },
  },
})
execFileSync(process.execPath, [resolve(root, 'web/.generated/asset-manifest.mjs'), root], { stdio: 'inherit' })
console.log('Web Demo 已准备好。启动：cargo run --manifest-path hsr-relic-web/Cargo.toml')
