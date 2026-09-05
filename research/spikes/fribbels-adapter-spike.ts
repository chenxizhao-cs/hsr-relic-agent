import { BasicKey } from 'lib/optimization/basicStatsArray'
import { generateContext } from 'lib/optimization/context/calculateContext'
import { RelicFilters } from 'lib/relics/relicFilters'
import { RelicAugmenter } from 'lib/relics/relicAugmenter'
import { RelicScorer } from 'lib/relics/scoring/relicScorer'
import { simulateBuild } from 'lib/simulations/simulateBuild'
import { generateFullDefaultForm } from 'lib/simulations/utils/benchmarkForm'
import { Metadata } from 'lib/state/metadataInitializer'
import { normalizeForm } from 'lib/stores/optimizerForm/optimizerFormConversions'
import type { SimulationRelicByPart } from 'lib/simulations/statSimulationTypes'
import type { Relic, UnaugmentedRelic } from 'types/relic'

type Input = {
  characterId: string
  lightConeId: string
  relics: Array<{
    id: string
    part: Relic['part']
    set: Relic['set']
    main: Relic['main']['stat']
    substats: [Relic['substats'][number]['stat'], number][]
  }>
}

async function readStdin(): Promise<string> {
  const chunks: Buffer[] = []
  for await (const chunk of process.stdin) chunks.push(Buffer.from(chunk))
  return Buffer.concat(chunks).toString('utf8')
}

Metadata.initialize()
const input = JSON.parse(await readStdin()) as Input

const relics = input.relics.map((item, ageIndex) => {
  const source: UnaugmentedRelic = {
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
  const relic = RelicAugmenter.augment(source)
  if (!relic) throw new Error(`Could not augment relic ${item.id}`)
  return relic
})

RelicFilters.condenseRelicSubstatsForOptimizerSingle(relics)
const byPart = RelicFilters.splitRelicsByPart(relics)
const single = Object.fromEntries(Object.entries(byPart).map(([part, values]) => [part, values[0]])) as SimulationRelicByPart
const form = normalizeForm(generateFullDefaultForm(input.characterId as never, input.lightConeId as never, 0, 1))
const result = simulateBuild(single, generateContext(form), null)

const scores = relics.map((relic) => {
  const current = RelicScorer.scoreCurrentRelic(relic, input.characterId as never)
  const potential = RelicScorer.scoreRelicPotential(relic, input.characterId as never)
  return {
    id: relic.id,
    percentScore: current.percentScore,
    rating: current.rating,
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

process.stdout.write(JSON.stringify({
  scores,
  panel: {
    hp: result.x.c.a[BasicKey.HP],
    atk: result.x.c.a[BasicKey.ATK],
    def: result.x.c.a[BasicKey.DEF],
    spd: result.x.c.a[BasicKey.SPD],
    cr: result.x.c.a[BasicKey.CR],
    cd: result.x.c.a[BasicKey.CD],
  },
  actionDamage: result.actionDamage,
  rotationDamage: result.rotationDamage,
  primaryActionStats: result.primaryActionStats,
}))
