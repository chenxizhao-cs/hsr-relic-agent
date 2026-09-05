// @vitest-environment jsdom
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { KelzScannerConfig } from 'lib/importer/importConfig'
import { KelzFormatParser, type ScannerParserJson } from 'lib/importer/kelzFormatParser'
import { RelicScorer } from 'lib/relics/scoring/relicScorer'
import { Metadata } from 'lib/state/metadataInitializer'
import type { CharacterId } from 'types/character'
import type { Relic } from 'types/relic'
import { describe, expect, test, vi } from 'vitest'

vi.mock('lib/interactions/message', () => ({
  Message: { warning: vi.fn(), error: vi.fn(), success: vi.fn() },
}))

vi.mock('i18next', () => ({
  default: {
    getFixedT: () => (key: string) => key,
    t: (key: string) => key,
  },
}))

Metadata.initialize()

function load(name: string): ScannerParserJson {
  return JSON.parse(readFileSync(resolve(import.meta.dirname, '../../fixtures', name), 'utf8')) as ScannerParserJson
}

function parse(name: string) {
  return new KelzFormatParser(KelzScannerConfig).parse(load(name))
}

function relicById(relics: Relic[], id: string): Relic {
  const relic = relics.find((item) => item.id === id)
  if (!relic) throw new Error(`Missing relic ${id}`)
  return relic
}

function score(relic: Relic, characterId: CharacterId) {
  const current = RelicScorer.scoreCurrentRelic(relic, characterId)
  const potential = RelicScorer.scoreRelicPotential(relic, characterId)
  return {
    current: current.percentScore,
    rating: current.rating,
    average: potential.averagePct,
    best: potential.bestPct,
  }
}

describe('HSR-Scanner v4 fixtures', () => {
  test('minimal fixture is accepted by the current KelzFormatParser', () => {
    const parsed = parse('scanner-v4-minimal.json')

    expect(parsed.characters).toHaveLength(1)
    expect(parsed.characters[0]).toMatchObject({
      characterId: '1205',
      characterLevel: 80,
      characterEidolon: 0,
      lightCone: '23009',
      lightConeLevel: 80,
      lightConeSuperimposition: 1,
    })
    expect(parsed.relics).toHaveLength(3)
    expect(parsed.relics.map((relic) => relic.id)).toEqual(['8100001', '8100002', '8100003'])
    expect(parsed.relics[0].equippedBy).toBe('1205')
    expect(parsed.relics.every((relic) => relic.main.stat && relic.substats.every((substat) => substat.stat))).toBe(true)
  })

  test('demo fixture supports staged upgrade decisions and target switching', () => {
    const parsed = parse('scanner-v4-demo.json')

    expect(parsed.characters.map((character) => character.characterId)).toEqual(['1205', '1102'])
    expect(parsed.characters.map((character) => character.lightCone)).toEqual(['23009', '23001'])
    expect(parsed.relics).toHaveLength(12)
    expect(new Set(parsed.relics.map((relic) => relic.enhance))).toEqual(new Set([0, 3, 6, 9, 12]))
    expect(relicById(parsed.relics, '9100001').previewSubstats).toHaveLength(1)

    const bladeGood = score(relicById(parsed.relics, '9100002'), '1205')
    const bladeStop = score(relicById(parsed.relics, '9100003'), '1205')
    const seeleGood = score(relicById(parsed.relics, '9200002'), '1102')
    const seeleStop = score(relicById(parsed.relics, '9200003'), '1102')
    expect(bladeGood.average).toBeGreaterThan(bladeStop.average)
    expect(seeleGood.average).toBeGreaterThan(seeleStop.average)

    const bladeCandidateForBlade = score(relicById(parsed.relics, '9100002'), '1205')
    const seeleCandidateForBlade = score(relicById(parsed.relics, '9200002'), '1205')
    const bladeCandidateForSeele = score(relicById(parsed.relics, '9100002'), '1102')
    const seeleCandidateForSeele = score(relicById(parsed.relics, '9200002'), '1102')
    expect(bladeCandidateForBlade.current).toBeGreaterThan(seeleCandidateForBlade.current)
    expect(seeleCandidateForSeele.current).toBeGreaterThan(bladeCandidateForSeele.current)

    console.log('SCANNER_V4_DEMO_SCORES', JSON.stringify({
      blade: { good: bladeGood, stop: bladeStop },
      seele: { good: seeleGood, stop: seeleStop },
      targetSwitch: {
        bladeTarget: { bladeCandidate: bladeCandidateForBlade.current, seeleCandidate: seeleCandidateForBlade.current },
        seeleTarget: { bladeCandidate: bladeCandidateForSeele.current, seeleCandidate: seeleCandidateForSeele.current },
      },
    }))
  })
})
