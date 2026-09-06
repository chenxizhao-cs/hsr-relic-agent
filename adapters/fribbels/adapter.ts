// Our protocol translation only. All scoring and simulation are upstream calls.
import gameData from 'data/game_data.json'
import { Parts, PartsMainStats, Stats } from 'lib/constants/constants'
import { BasicKey } from 'lib/optimization/basicStatsArray'
import { generateContext } from 'lib/optimization/context/calculateContext'
import { RelicFilters } from 'lib/relics/relicFilters'
import { RelicAugmenter } from 'lib/relics/relicAugmenter'
import { RelicScorer } from 'lib/relics/scoring/relicScorer'
import { simulateBuild } from 'lib/simulations/simulateBuild'
import { generateFullDefaultForm } from 'lib/simulations/utils/benchmarkForm'
import { Metadata } from 'lib/state/metadataInitializer'
import { getGameMetadata } from 'lib/state/gameMetadata'
import { normalizeForm } from 'lib/stores/optimizerForm/optimizerFormConversions'
import type { SimulationRelicByPart } from 'lib/simulations/statSimulationTypes'
import type { CharacterId } from 'types/character'
import type { LightConeId } from 'types/lightCone'
import type { UnaugmentedRelic } from 'types/relic'

declare const __UPSTREAM_COMMIT__: string
const stats = {
  hp: Stats.HP, atk: Stats.ATK, def: Stats.DEF,
  hp_percent: Stats.HP_P, atk_percent: Stats.ATK_P, def_percent: Stats.DEF_P,
  speed: Stats.SPD, crit_rate: Stats.CR, crit_damage: Stats.CD,
  effect_hit: Stats.EHR, effect_res: Stats.RES, break_effect: Stats.BE,
  energy_regen: Stats.ERR, healing: Stats.OHB,
  physical_damage: Stats.Physical_DMG, fire_damage: Stats.Fire_DMG, ice_damage: Stats.Ice_DMG,
  lightning_damage: Stats.Lightning_DMG, wind_damage: Stats.Wind_DMG,
  quantum_damage: Stats.Quantum_DMG, imaginary_damage: Stats.Imaginary_DMG,
} as const
const parts = { head: Parts.Head, hands: Parts.Hands, body: Parts.Body, feet: Parts.Feet,
  sphere: Parts.PlanarSphere, rope: Parts.LinkRope } as const
type RelicInput = {
  id: string, slot: keyof typeof parts, set_id: string, rarity: number, level: number,
  main_stat: keyof typeof stats, substats: Record<keyof typeof stats, number>,
}
type Input = {
  schema_version: number,
  character: { id: string, level: number, eidolon: number,
    light_cone: { id: string, level: number, superimposition: number } },
  relics: RelicInput[], builds: { id: string, relic_ids: string[] }[],
  conditions: { preset: string, enemy_level: number, enemy_resistance_pct: number,
    elemental_weakness: boolean, weakness_broken: boolean },
}
function check(ok: unknown, message: string): asserts ok {
  if (!ok) throw new Error(message)
}
const finite = (n: unknown): n is number => typeof n === 'number' && Number.isFinite(n)
const integer = (n: unknown, min: number, max: number) => finite(n) && Number.isInteger(n) && n >= min && n <= max

function evaluate(input: Input) {
  check(input.schema_version === 1, 'Unsupported schema_version')
  const c = input.character
  check(c && ['1205', '1102'].includes(c.id), 'v0.2 supports unbuffed Blade / Seele only')
  check(c.level === 80 && c.light_cone?.level === 80, 'v0.2 requires level 80 character and light cone')
  check(integer(c.eidolon, 0, 6) && integer(c.light_cone.superimposition, 1, 5), 'Invalid eidolon/superimposition')
  const metadata = getGameMetadata()
  check(metadata.lightCones[c.light_cone.id as LightConeId]?.path === metadata.characters[c.id as CharacterId].path, 'Unknown or incompatible light cone')
  const conditions = input.conditions
  check(conditions?.preset === 'solo-default-v1', 'Unsupported combat preset')
  check(integer(conditions.enemy_level, 1, 100)
    && finite(conditions.enemy_resistance_pct) && conditions.enemy_resistance_pct >= 0 && conditions.enemy_resistance_pct <= 100
    && typeof conditions.elemental_weakness === 'boolean' && typeof conditions.weakness_broken === 'boolean', 'Invalid enemy conditions')
  check(Array.isArray(input.relics) && input.relics.length > 0 && input.relics.length <= 2000, 'Invalid relic batch size')
  check(Array.isArray(input.builds) && input.builds.length <= 2001, 'Invalid build batch size')
  const ids = new Set<string>()
  const relics = input.relics.map((item, ageIndex) => {
    check(typeof item.id === 'string' && item.id.length > 0 && !ids.has(item.id), 'Invalid/duplicate relic id')
    ids.add(item.id)
    const part = parts[item.slot]
    const main = stats[item.main_stat]
    const set = gameData.relics.find(set => set.id === item.set_id)
    check(part && main && set, `Unknown slot/stat/set for ${item.id}`)
    check((Number(set.id) >= 300) === ['sphere', 'rope'].includes(item.slot), 'Set/slot mismatch')
    check(PartsMainStats[part].includes(main as never), 'Invalid main stat for slot')
    check(item.rarity === 5 && integer(item.level, 0, 15) && item.level % 3 === 0, 'Expected five-star +3 checkpoint')
    const subs = Object.entries(item.substats)
    check(subs.length >= 3 && subs.length <= 4 && (item.level === 0 || subs.length === 4), 'Invalid substat count')
    const substats = subs.map(([name, value]) => {
      check(Object.keys(stats).slice(0, 12).includes(name) && name !== item.main_stat && finite(value) && value > 0, 'Invalid substat')
      return { stat: stats[name as keyof typeof stats], value }
    })
    const source = { id: item.id, ageIndex, enhance: item.level, grade: item.rarity, part,
      set: set.name, main: { stat: main, value: 0 }, substats, previewSubstats: [], equippedBy: undefined } as UnaugmentedRelic
    const relic = RelicAugmenter.augment(source)
    check(relic, `Could not augment ${item.id}`)
    return relic
  })
  // Fresh scorer per relic; its upstream cache otherwise keys by ID, not observed stats.
  const scores = relics.map(relic => {
    const score = RelicScorer.scoreCurrentRelic(relic, c.id as CharacterId)
    const p = RelicScorer.scoreRelicPotential(relic, c.id as CharacterId)
    return { id: relic.id, raw_current_score: score.percentScore, rating: score.rating,
      current: p.currentPct, average: p.averagePct, best: p.bestPct, worst: p.worstPct }
  })
  const buildIds = new Set<string>()
  const builds = input.builds.map(build => {
    check(typeof build.id === 'string' && !buildIds.has(build.id), 'Invalid/duplicate build id')
    buildIds.add(build.id)
    check(Array.isArray(build.relic_ids) && build.relic_ids.length === 6 && new Set(build.relic_ids).size === 6, 'Build requires six distinct relics')
    const selected = build.relic_ids.map(id => {
      const relic = relics.find(relic => relic.id === id)
      check(relic, `Missing build relic ${id}`)
      return structuredClone(relic)
    })
    check(new Set(selected.map(relic => relic.part)).size === 6, 'Build requires all six slots')
    // Keep actual enhancement levels. Do not applyMainStatsFilter (which can upscale).
    RelicFilters.condenseRelicSubstatsForOptimizerSingle(selected)
    const single = Object.fromEntries(selected.map(relic => [relic.part, relic])) as unknown as SimulationRelicByPart
    const form = normalizeForm(generateFullDefaultForm(c.id as CharacterId, c.light_cone.id as LightConeId, c.eidolon, c.light_cone.superimposition))
    form.enemyLevel = conditions.enemy_level
    form.enemyCount = 1
    form.enemyResistance = conditions.enemy_resistance_pct / 100
    form.enemyElementalWeak = conditions.elemental_weakness
    form.enemyWeaknessBroken = conditions.weakness_broken
    form.enemyMaxToughness = 360
    form.enemyEffectResistance = 0.3
    form.mainStatUpscaleLevel = 0
    // Default form has no teammates; keep upstream's pinned character/LC/set toggles.
    const result = simulateBuild(single, generateContext(form), null)
    const a = result.x.c.a
    return { id: build.id, panel: { hp: a[BasicKey.HP], atk: a[BasicKey.ATK], def: a[BasicKey.DEF],
      speed: a[BasicKey.SPD], crit_rate_pct: a[BasicKey.CR] * 100, crit_damage_pct: a[BasicKey.CD] * 100 },
      damage_model: 'legacy_atk_basic_v1', basic_damage: result.actionDamage?.BASIC }
  })
  return { scores, builds }
}

// Reserve stdout for a single protocol response, including during upstream initialization.
console.log = (...args) => console.error(...args)
try {
  let text = ''
  for await (const chunk of process.stdin) {
    text += chunk.toString()
    check(Buffer.byteLength(text) <= 4 * 1024 * 1024, 'Input exceeds 4 MiB')
  }
  Metadata.initialize()
  const result = evaluate(JSON.parse(text))
  const response = { schema_version: 1, upstream_commit: __UPSTREAM_COMMIT__, ok: true, result }
  // JSON.stringify silently converts NaN/Infinity to null: reject instead.
  const json = JSON.stringify(response, (_key, value) => {
    if (typeof value === 'number') check(Number.isFinite(value), 'Non-finite upstream metric')
    return value
  })
  process.stdout.write(json + '\n')
} catch (error) {
  process.stdout.write(JSON.stringify({ schema_version: 1, upstream_commit: __UPSTREAM_COMMIT__, ok: false,
    error: { code: 'evaluation_failed', message: error instanceof Error ? error.message : String(error) } }) + '\n')
  process.exitCode = 1
}
