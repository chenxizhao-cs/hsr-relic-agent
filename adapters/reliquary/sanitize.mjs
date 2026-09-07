import { readFile, rename, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { mkdir } from 'node:fs/promises'

const [inputArg, outputArg] = process.argv.slice(2)
if (!inputArg || !outputArg) {
  throw new Error('Usage: node adapters/reliquary/sanitize.mjs <private-input.json> <sanitized-output.json>')
}

const input = resolve(inputArg)
const output = resolve(outputArg)
const raw = JSON.parse(await readFile(input, 'utf8'))

if (raw?.source !== 'reliquary_archiver' || raw?.version !== 4) {
  throw new Error('Expected Reliquary Archiver v4 JSON')
}
for (const field of ['characters', 'relics', 'light_cones', 'materials']) {
  if (!Array.isArray(raw[field])) throw new Error(`Expected ${field} array`)
}
if (!raw.metadata || typeof raw.metadata !== 'object' || !raw.gacha || typeof raw.gacha !== 'object') {
  throw new Error('Expected metadata and gacha objects')
}

const relics = raw.relics.map((relic, index) => ({
  ...relic,
  _uid: String(900000000 + index + 1),
}))
const lightCones = raw.light_cones.map((lightCone, index) => ({
  ...lightCone,
  _uid: String(800000000 + index + 1),
}))
const sanitized = {
  source: raw.source,
  build: raw.build,
  version: raw.version,
  metadata: {
    uid: null,
    trailblazer: null,
  },
  gacha: {
    stellar_jade: 0,
    oneric_shards: 0,
  },
  materials: [],
  light_cones: lightCones,
  relics,
  characters: raw.characters,
}

if (new Set(relics.map((relic) => relic._uid)).size !== relics.length) {
  throw new Error('Sanitized relic IDs are not unique')
}
if (new Set(lightCones.map((lightCone) => lightCone._uid)).size !== lightCones.length) {
  throw new Error('Sanitized light cone IDs are not unique')
}

await mkdir(dirname(output), { recursive: true })
const temporary = `${output}.tmp`
await writeFile(temporary, `${JSON.stringify(sanitized)}\n`, { mode: 0o600 })
await rename(temporary, output)
console.log(`已生成脱敏 Reliquary Demo：${sanitized.characters.length} 个角色 / ${relics.length} 件遗器 / ${lightCones.length} 个光锥 → ${output}`)
