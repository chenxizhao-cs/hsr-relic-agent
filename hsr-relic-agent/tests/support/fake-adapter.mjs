// Fault injection only; never selected by the application or Fribbels evaluator defaults.
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
let input = ''
for await (const chunk of process.stdin) input += chunk
const request = JSON.parse(input)
const mode = request.conditions.enemy_level
const envelope = { schema_version: 1, upstream_commit: 'df630a0488a64eeb740e4e0c14f265d96b9f6f8f', ok: true }
if (mode === 91) {
  process.stdout.write('not JSON')
} else if (mode === 92) {
  process.stdout.write(JSON.stringify({ ...envelope, upstream_commit: 'wrong-version' }))
} else if (mode === 93) {
  process.stdout.write(JSON.stringify(envelope))
  process.exitCode = 2
} else if (mode === 94) {
  setInterval(() => {}, 1000)
} else if (mode === 95 && request.relics.find(r => r.id === '9100002').level >= 6) {
  process.stdout.write(JSON.stringify({ ...envelope, ok: false, error: { code: 'injected', message: 'staged evaluation failed' } }))
  process.exitCode = 1
} else {
  const adapter = fileURLToPath(new URL('../../../adapters/fribbels/dist/adapter.mjs', import.meta.url))
  const real = JSON.parse(execFileSync(process.execPath, [adapter], { input, encoding: 'utf8', timeout: 5000 }))
  if (mode === 96) real.result.builds.pop()
  if (mode === 97) real.result.scores[0].average = -1
  if (mode === 98) real.result.scores[1] = real.result.scores[0]
  if (mode === 99) real.result.builds[0].panel.hp = null
  process.stdout.write(JSON.stringify(real))
}
