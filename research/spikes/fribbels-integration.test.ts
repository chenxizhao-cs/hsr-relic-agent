// @vitest-environment jsdom
import { Constants, Parts, Stats } from 'lib/constants/constants'
import { FixedSizeMinQueue } from 'lib/dataStructures/fixedSizeMinQueue'
import { BufferPacker, type OptimizerDisplayData } from 'lib/optimization/bufferPacker'
import { generateContext } from 'lib/optimization/context/calculateContext'
import { generateOrnamentSetSolutions, generateRelicSetSolutions } from 'lib/optimization/relicSetSolver'
import { bitpackBooleanArray } from 'lib/optimization/setSolutionBitset'
import { RelicFilters } from 'lib/relics/relicFilters'
import { RelicAugmenter } from 'lib/relics/relicAugmenter'
import { RelicScorer } from 'lib/relics/scoring/relicScorer'
import { simulateBuild } from 'lib/simulations/simulateBuild'
import { generateFullDefaultForm } from 'lib/simulations/utils/benchmarkForm'
import { Metadata } from 'lib/state/metadataInitializer'
import { normalizeForm } from 'lib/stores/optimizerForm/optimizerFormConversions'
import { useRelicStore } from 'lib/stores/relic/relicStore'
import { optimizerWorker } from 'lib/worker/optimizerWorker'
import { WorkerType } from 'lib/worker/workerUtils'
import type { RelicsByPart } from 'lib/gpu/webgpuTypes'
import type { SimulationRelicByPart } from 'lib/simulations/statSimulationTypes'
import type { Relic, UnaugmentedRelic } from 'types/relic'
import { describe, expect, test, vi } from 'vitest'

Metadata.initialize()

const INPUT = JSON.parse(JSON.stringify({
  characterId: '1205',
  lightConeId: '23009',
  relics: [
    { id: 'h1', part: Parts.Head, set: 'Longevous Disciple', main: Stats.HP, substats: [[Stats.CR, 6.4], [Stats.CD, 12.9], [Stats.SPD, 5], [Stats.HP_P, 8.6]] },
    { id: 'g1', part: Parts.Hands, set: 'Longevous Disciple', main: Stats.ATK, substats: [[Stats.CR, 5.8], [Stats.CD, 11.6], [Stats.SPD, 4], [Stats.HP_P, 9.2]] },
    { id: 'b1', part: Parts.Body, set: 'Longevous Disciple', main: Stats.CR, substats: [[Stats.CD, 18.1], [Stats.SPD, 4], [Stats.HP_P, 9.2], [Stats.ATK_P, 4.3]] },
    { id: 'f1', part: Parts.Feet, set: 'Longevous Disciple', main: Stats.SPD, substats: [[Stats.CR, 5.8], [Stats.CD, 12.3], [Stats.HP_P, 8.6], [Stats.ATK_P, 4.3]] },
    { id: 'p1', part: Parts.PlanarSphere, set: 'Inert Salsotto', main: Stats.Wind_DMG, substats: [[Stats.CR, 5.8], [Stats.CD, 12.3], [Stats.SPD, 4], [Stats.HP_P, 8.6]] },
    { id: 'l1', part: Parts.LinkRope, set: 'Inert Salsotto', main: Stats.HP_P, substats: [[Stats.CR, 5.8], [Stats.CD, 12.3], [Stats.SPD, 4], [Stats.ATK_P, 8.6]] },
  ],
})) as {
  characterId: string
  lightConeId: string
  relics: Array<{ id: string, part: Relic['part'], set: Relic['set'], main: Relic['main']['stat'], substats: [Relic['substats'][number]['stat'], number][] }>
}

function augmentInput(): Relic[] {
  return INPUT.relics.map((item, ageIndex) => {
    const relic: UnaugmentedRelic = {
      id: item.id,
      ageIndex,
      enhance: 15,
      grade: 5,
      part: item.part,
      set: item.set,
      main: { stat: item.main, value: 0 },
      substats: item.substats.map(([stat, value]) => ({ stat, value })),
      previewSubstats: [],
      equippedBy: undefined,
    }
    const augmented = RelicAugmenter.augment(relic)
    if (!augmented) throw new Error(`Could not augment ${item.id}`)
    return augmented
  })
}

function prepare() {
  const relics = augmentInput()
  RelicFilters.condenseRelicSubstatsForOptimizerSingle(relics)
  const byPart = RelicFilters.splitRelicsByPart(relics) as RelicsByPart
  const single = Object.fromEntries(Object.entries(byPart).map(([part, values]) => [part, values[0]])) as SimulationRelicByPart
  const form = normalizeForm(generateFullDefaultForm(INPUT.characterId as never, INPUT.lightConeId as never, 0, 1))
  form.enhance = 0
  form.grade = 0
  form.rankFilter = false
  return { relics, byPart, single, form, context: generateContext(form) }
}

describe('Fribbels integration spike', () => {
  test('accepts caller-owned structured data and returns JSON-safe relic scores', () => {
    const { relics } = prepare()
    const scores = relics.map((relic) => {
      const current = RelicScorer.scoreCurrentRelic(relic, INPUT.characterId as never)
      const potential = RelicScorer.scoreRelicPotential(relic, INPUT.characterId as never)
      return {
        id: relic.id,
        current: { percentScore: current.percentScore, rating: current.rating },
        potential: {
          currentPct: potential.currentPct,
          bestPct: potential.bestPct,
          averagePct: potential.averagePct,
          worstPct: potential.worstPct,
          rerollAvgPct: potential.rerollAvgPct,
          blockedRerollAvgPct: potential.blockedRerollAvgPct,
        },
      }
    })

    expect(scores).toHaveLength(6)
    expect(scores.every((score) => Number.isFinite(score.current.percentScore))).toBe(true)
    expect(JSON.parse(JSON.stringify(scores))).toEqual(scores)
    console.log('SPIKE_SCORE_OUTPUT', JSON.stringify(scores))
  })

  test('simulateBuild runs without mounted UI and yields panel and damage values', () => {
    const { single, context } = prepare()
    const result = simulateBuild(single, context, null)
    const output = {
      basic: {
        hp: result.x.c.a[4],
        atk: result.x.c.a[5],
        def: result.x.c.a[6],
        spd: result.x.c.a[7],
        cr: result.x.c.a[8],
        cd: result.x.c.a[9],
      },
      combo: result.rotationDamage?.reduce((sum, step) => sum + step.damage, 0),
      actionDamage: result.actionDamage,
      rotationDamage: result.rotationDamage,
      primaryActionStats: result.primaryActionStats,
    }

    expect(output.basic.hp).toBeGreaterThan(0)
    expect(output.combo).toBeGreaterThan(0)
    expect(JSON.parse(JSON.stringify(output))).toEqual(output)
    console.log('SPIKE_SIM_OUTPUT', JSON.stringify(output))
  })

  test('the worker optimizer kernel can run synchronously and its packed result can be decoded', () => {
    const { byPart, form, context } = prepare()
    const postMessage = vi.spyOn(self, 'postMessage').mockImplementation(() => undefined)
    const buffer = BufferPacker.createFloatBuffer(1)
    const input = {
      workerType: WorkerType.OPTIMIZER,
      relics: byPart,
      request: { ...form, resultMinFilter: 0 },
      context,
      buffer,
      relicSetSolutions: bitpackBooleanArray(generateRelicSetSolutions(form)),
      ornamentSetSolutions: bitpackBooleanArray(generateOrnamentSetSolutions(form)),
      permutations: 1,
      WIDTH: 1,
      skip: 0,
    }

    optimizerWorker({ data: input } as MessageEvent<typeof input>)
    expect(postMessage).toHaveBeenCalledOnce()
    const outputBuffer = (postMessage.mock.calls[0][0] as { buffer: ArrayBuffer }).buffer
    const row = BufferPacker.extractCharacter(new Float32Array(outputBuffer), 0, 0)
    expect(row.HP).toBeGreaterThan(0)
    expect(row.COMBO).toBeGreaterThan(0)
    expect(JSON.parse(JSON.stringify({ id: row.id, HP: row.HP, SPD: row.SPD, CR: row.CR, CD: row.CD, COMBO: row.COMBO }))).toBeTruthy()
    console.log('SPIKE_OPTIMIZER_KERNEL_OUTPUT', JSON.stringify({ id: row.id, HP: row.HP, SPD: row.SPD, CR: row.CR, CD: row.CD, COMBO: row.COMBO }))
    postMessage.mockRestore()
  })

  test('Optimizer facade filters store data but optimize does not expose candidate rows as a return value', async () => {
    const { relics, form } = prepare()
    useRelicStore.getState().setRelics(relics)
    const { Optimizer } = await import('lib/optimization/optimizer')
    const [filtered] = Optimizer.getFilteredRelics(form)
    const counts = Object.fromEntries(Object.entries(filtered).map(([part, values]) => [part, values.length]))
    expect(counts).toEqual({ Head: 1, Hands: 1, Body: 1, Feet: 1, PlanarSphere: 1, LinkRope: 1 })
    useRelicStore.getState().setRelics([])
    const returnValue = await Optimizer.optimize(form)
    expect(returnValue).toBeUndefined()
    console.log('SPIKE_OPTIMIZER_FACADE', JSON.stringify({ counts, actualReturn: returnValue ?? null, returnContract: 'Promise<void>; rows are delivered through UI/store side effects' }))
  })
})
