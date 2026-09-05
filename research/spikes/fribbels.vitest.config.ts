import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig } from '../../upstream/hsr-optimizer/node_modules/vitest/dist/config.js'

const workspace = resolve(fileURLToPath(new URL('.', import.meta.url)), '../..')
const upstream = resolve(workspace, 'upstream/hsr-optimizer')

export default defineConfig({
  root: workspace,
  resolve: {
    alias: [
      { find: 'cross-fetch', replacement: resolve(upstream, 'src/lib/utils/nativeFetch.ts') },
      { find: /^lib\/(.*)$/, replacement: `${resolve(upstream, 'src/lib')}/$1` },
      { find: /^types\/(.*)$/, replacement: `${resolve(upstream, 'src/types')}/$1` },
      { find: /^data\/(.*)$/, replacement: `${resolve(upstream, 'src/data')}/$1` },
      { find: /^style\/(.*)$/, replacement: `${resolve(upstream, 'src/style')}/$1` },
      { find: /^icons\/(.*)$/, replacement: `${resolve(upstream, 'src/icons')}/$1` },
    ],
  },
  test: {
    include: [
      'research/spikes/fribbels-integration.test.ts',
      'research/spikes/scanner-v4-fixtures.test.ts',
    ],
    environment: 'node',
    execArgv: ['--no-webstorage'],
  },
})
