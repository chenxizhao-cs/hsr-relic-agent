import { createHash } from 'node:crypto'
import { mkdir, rename, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const SOURCE_COMMIT = '8cdb905dc2f8e6fffa9be4eb07af3e34435d6091'
const SOURCE_PATH = 'ExcelOutput/AvatarRelicRecommend.json'
const SOURCE_SHA256 = '26e5b39b7471abc4c1f51481611dfd890d8cd108869d129b05bd9bfa23408447'
const SOURCE_REPOSITORY = 'https://github.com/DimbreathBot/TurnBasedGameData'
const SOURCE_URL = `https://raw.githubusercontent.com/DimbreathBot/TurnBasedGameData/${SOURCE_COMMIT}/${SOURCE_PATH}`

const statNames = {
  AttackAddedRatio: 'atk_percent',
  AttackDelta: 'atk',
  BreakDamageAddedRatioBase: 'break_effect',
  CriticalChanceBase: 'crit_rate',
  CriticalDamageBase: 'crit_damage',
  DefenceAddedRatio: 'def_percent',
  FireAddedRatio: 'fire_damage',
  HPAddedRatio: 'hp_percent',
  HealRatioBase: 'healing',
  IceAddedRatio: 'ice_damage',
  ImaginaryAddedRatio: 'imaginary_damage',
  PhysicalAddedRatio: 'physical_damage',
  QuantumAddedRatio: 'quantum_damage',
  SPRatioBase: 'energy_regen',
  SpeedDelta: 'speed',
  StatusProbabilityBase: 'effect_hit',
  StatusResistanceBase: 'effect_res',
  ThunderAddedRatio: 'lightning_damage',
  WindAddedRatio: 'wind_damage',
}

function requireArray(row, field) {
  const value = row[field]
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error(`角色 ${row.AvatarID ?? '?'} 缺少非空 ${field}`)
  }
  return value
}

function unique(values, label) {
  const result = [...new Set(values)]
  if (result.length !== values.length) throw new Error(`${label} 包含重复项`)
  return result
}

function sets(row, field) {
  return unique(requireArray(row, field).map((value) => {
    if (!Number.isInteger(value) || value <= 0) throw new Error(`角色 ${row.AvatarID} 的 ${field} 含无效 ID`)
    return String(value)
  }), `角色 ${row.AvatarID} 的 ${field}`)
}

function stats(row, field) {
  return unique(requireArray(row, field).map((value) => {
    const mapped = statNames[value]
    if (!mapped) throw new Error(`角色 ${row.AvatarID} 的 ${field} 含未知属性 ${value}`)
    return mapped
  }), `角色 ${row.AvatarID} 的 ${field}`)
}

const root = fileURLToPath(new URL('../..', import.meta.url))
const output = resolve(root, 'data/.generated/character-relic-recommendations-v1.json')
const response = await fetch(SOURCE_URL)
if (!response.ok) throw new Error(`下载静态推荐表失败：HTTP ${response.status}`)
const bytes = Buffer.from(await response.arrayBuffer())
const actualSha256 = createHash('sha256').update(bytes).digest('hex')
if (actualSha256 !== SOURCE_SHA256) {
  throw new Error(`静态推荐表校验失败：期望 ${SOURCE_SHA256}，实际 ${actualSha256}`)
}

const rows = JSON.parse(bytes.toString('utf8'))
if (!Array.isArray(rows) || rows.length !== 93) {
  throw new Error(`静态推荐表记录数异常：期望 93，实际 ${Array.isArray(rows) ? rows.length : '非数组'}`)
}
const seen = new Set()
const profiles = rows.map((row) => {
  if (!Number.isInteger(row.AvatarID) || row.AvatarID <= 0) throw new Error('发现无效 AvatarID')
  const characterId = String(row.AvatarID)
  if (seen.has(characterId)) throw new Error(`重复 AvatarID：${characterId}`)
  seen.add(characterId)
  return {
    character_id: characterId,
    relic_set_ids: sets(row, 'Set4IDList'),
    ornament_set_ids: sets(row, 'Set2IDList'),
    main_stats: {
      body: stats(row, 'PropertyList3'),
      feet: stats(row, 'PropertyList4'),
      sphere: stats(row, 'PropertyList5'),
      rope: stats(row, 'PropertyList6'),
    },
    substats: stats(row, 'SubAffixPropertyList'),
  }
}).sort((a, b) => Number(a.character_id) - Number(b.character_id))

const document = {
  schema_version: 1,
  source: {
    kind: 'honkai_star_rail_client_config',
    repository_url: SOURCE_REPOSITORY,
    commit: SOURCE_COMMIT,
    path: SOURCE_PATH,
    sha256: SOURCE_SHA256,
  },
  profiles,
}

await mkdir(dirname(output), { recursive: true })
const temporary = `${output}.tmp`
await writeFile(temporary, `${JSON.stringify(document, null, 2)}\n`, 'utf8')
await rename(temporary, output)
console.log(`静态推荐数据库已生成：${profiles.length} 个角色 → ${output}`)
