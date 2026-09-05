import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig } from '../../upstream/hsr-optimizer/node_modules/vite/dist/node/index.js'

const workspace = resolve(fileURLToPath(new URL('.', import.meta.url)), '../..')
const upstream = resolve(workspace, 'upstream/hsr-optimizer')

export default defineConfig({
  root: workspace,
  publicDir: false,
  resolve: {
    alias: [
      { find: 'cross-fetch', replacement: resolve(upstream, 'src/lib/utils/nativeFetch.ts') },
      { find: /^lib\/(.*)$/, replacement: `${resolve(upstream, 'src/lib')}/$1` },
      { find: /^types\/(.*)$/, replacement: `${resolve(upstream, 'src/types')}/$1` },
      { find: /^data\/(.*)$/, replacement: `${resolve(upstream, 'src/data')}/$1` },
    ],
  },
  ssr: { noExternal: true },
  build: {
    ssr: resolve(workspace, 'research/spikes/fribbels-adapter-spike.ts'),
    outDir: '/tmp/fribbels-adapter-spike',
    emptyOutDir: true,
    target: 'node26',
    minify: false,
    sourcemap: false,
    rollupOptions: {
      output: { entryFileNames: 'adapter.mjs' },
    },
  },
})
