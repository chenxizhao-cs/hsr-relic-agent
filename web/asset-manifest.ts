// Integration wrapper authored here. Calls Fribbels Assets instead of copying its URL rules.
import gameData from 'data/game_data.json'
import {
  Parts,
  Stats,
} from 'lib/constants/constants'
import { Assets } from 'lib/rendering/assets'
import {
  existsSync,
  readFileSync,
  writeFileSync,
} from 'node:fs'
import { resolve } from 'node:path'

const root = process.argv[2]
const fixture = JSON.parse(readFileSync(resolve(root, 'fixtures/scanner-v4-demo.json'), 'utf8'))
const local = (url: string) => {
  const path = new URL(url).pathname
  const relative = path.slice(path.indexOf('/assets/') + '/assets/'.length)
  if (!path.includes('/assets/') || !existsSync(resolve(root, 'upstream/hsr-optimizer/public/assets', relative))) {
    throw new Error(`Missing upstream visual asset: ${path}`)
  }
  return `/assets/${relative}`
}
const parts = { head: Parts.Head, hands: Parts.Hands, body: Parts.Body, feet: Parts.Feet, sphere: Parts.PlanarSphere, rope: Parts.LinkRope }
const stats = {
  hp: Stats.HP,
  atk: Stats.ATK,
  def: Stats.DEF,
  hp_percent: Stats.HP_P,
  atk_percent: Stats.ATK_P,
  def_percent: Stats.DEF_P,
  speed: Stats.SPD,
  crit_rate: Stats.CR,
  crit_damage: Stats.CD,
  effect_hit: Stats.EHR,
  effect_res: Stats.RES,
  break_effect: Stats.BE,
  energy_regen: Stats.ERR,
  healing: Stats.OHB,
  physical_damage: Stats.Physical_DMG,
  fire_damage: Stats.Fire_DMG,
  ice_damage: Stats.Ice_DMG,
  lightning_damage: Stats.Lightning_DMG,
  wind_damage: Stats.Wind_DMG,
  quantum_damage: Stats.Quantum_DMG,
  imaginary_damage: Stats.Imaginary_DMG,
}
const characters = Object.fromEntries(fixture.characters.map((c: { id: string }) => [c.id, {
  avatar: local(Assets.getCharacterAvatarById(c.id)),
  portrait: local(Assets.getCharacterPortraitById(c.id)),
  preview: local(Assets.getCharacterPreviewById(c.id)),
}]))
const relics = Object.fromEntries([...new Set(fixture.relics.map((r: { set_id: string }) => r.set_id))].map((id) => {
  const set = gameData.relics.find((r) => r.id === id)
  if (!set) throw new Error(`Unknown set ${id}`)
  // Only request slots actually present in the fixture; ornament/body sets differ.
  const usedSlots = fixture.relics.filter((r: { set_id: string }) => r.set_id === id)
    .map((r: { slot: string }) => r.slot.replaceAll(' ', '')).filter((v: string, i: number, all: string[]) => all.indexOf(v) === i)
  return [
    id,
    Object.fromEntries(
      Object.entries(parts).filter(([, part]) => usedSlots.includes(part))
        .map(([slot, part]) => [slot, local(Assets.getSetImage(set.name, part))]),
    ),
  ]
}))
const manifest = {
  source: 'Fribbels Assets',
  upstream_commit: 'df630a0488a64eeb740e4e0c14f265d96b9f6f8f',
  characters,
  relics,
  stats: Object.fromEntries(Object.entries(stats).map(([key, stat]) => [key, local(Assets.getStatIcon(stat))])),
  star: local(Assets.getStar()),
  fallback: local(Assets.getDefaultRelic()),
}
writeFileSync(resolve(root, 'web/.generated/assets.json'), JSON.stringify(manifest, null, 2) + '\n')
console.log(`Visual manifest ready: ${Object.keys(characters).length} characters, ${Object.keys(relics).length} relic sets; every asset exists locally.`)
