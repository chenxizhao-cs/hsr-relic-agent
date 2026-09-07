import { DemoApi } from './api.js'
import { AssetProvider } from './asset-provider.js'

const $ = (id) => document.getElementById(id)
const esc = (value) => String(value ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', '\'': '&#39;' }[c]))
const number = (value) => Number(value).toLocaleString('zh-CN', { maximumFractionDigits: 2 })
const names = { '1205': '刃', '1102': '希儿' }
const descriptions = { '1205': ['BLADE', '风 · 毁灭', '以手中之剑，回应下一次选择。'], '1102': ['SEELE', '量子 · 巡猎', '捕捉转瞬之间的每一次机会。'] }
const sets = { '113': '宝命长存的莳者', '108': '繁星璀璨的天才', '306': '停转的萨尔索图', '309': '繁星竞技场' }
const slots = { head: '头部', hands: '手部', body: '躯干', feet: '脚部', sphere: '位面球', rope: '连结绳' }
const stats = {
  hp: '生命值',
  atk: '攻击力',
  def: '防御力',
  hp_percent: '生命值%',
  atk_percent: '攻击力%',
  def_percent: '防御力%',
  speed: '速度',
  crit_rate: '暴击率',
  crit_damage: '暴击伤害',
  effect_hit: '效果命中',
  effect_res: '效果抵抗',
  break_effect: '击破特攻',
  energy_regen: '能量恢复效率',
  healing: '治疗量加成',
  physical_damage: '物理伤害加成',
  fire_damage: '火属性伤害加成',
  ice_damage: '冰属性伤害加成',
  lightning_damage: '雷属性伤害加成',
  wind_damage: '风属性伤害加成',
  quantum_damage: '量子属性伤害加成',
  imaginary_damage: '虚数属性伤害加成',
}
const inputs = ['hp', 'atk', 'def', 'hp_percent', 'atk_percent', 'def_percent', 'speed', 'crit_rate', 'crit_damage', 'effect_hit', 'effect_res', 'break_effect']
const labels = { Continue: '继续强化', Hold: '暂缓投入', Stop: '停止投入' }
const percent = (stat) => !['hp', 'atk', 'def', 'speed'].includes(stat)
const value = (stat, n) => `${number(n)}${percent(stat) ? '%' : ''}`
const api = new DemoApi()
let state, assets, tab = 'ranked', busy = false, timer, progressLabel = '正在处理'
let taskState = { active: null, tasks: [] }, liveRun = null, selectedTaskId = null, eventSource

function img(src, cls = '', alt = '') {
  return `<img src="${esc(src)}" class="${cls}" alt="${esc(alt)}" loading="lazy">`
}
document.addEventListener('error', (e) => {
  if (e.target.tagName === 'IMG' && !e.target.dataset.fallback) {
    e.target.dataset.fallback = 'true'
    e.target.src = assets.fallback()
  }
}, true)

function render() {
  $('budget').textContent = state.remaining_budget
  $('evaluator-label').textContent = state.evaluator === 'Mock' ? '◇ Mock 对照模式' : '✦ Fribbels 评价'
  $('reset').disabled = busy
  $('model-settings').disabled = busy
  $('save-session').disabled = busy
  $('load-session').disabled = busy
  $('characters').innerHTML = state.characters.map((c) =>
    `<button class="character-choice ${c.id === state.target_id ? 'selected' : ''}" data-target="${c.id}" ${busy ? 'disabled' : ''} aria-pressed="${
      c.id === state.target_id
    }">${img(assets.character(c.id), 'avatar', names[c.id])}<span>${names[c.id] ?? esc(c.name)}<small>Lv. ${c.level}</small></span><span class="choice-mark">${
      c.id === state.target_id ? '✦' : '◇'
    }</span></button>`
  ).join('')
  if (state.target_id) {
    const [name, path, line] = descriptions[state.target_id]
    $('character-art').innerHTML = `<div class="orbit"></div>${
      img(assets.character(state.target_id, 'portrait'), 'portrait', names[state.target_id])
    }<div class="character-code">${name}</div>`
    $('character-caption').innerHTML = `<small>${path}</small><h2>${names[state.target_id]} <span>${name}</span></h2><p>${line}</p>`
    document.body.dataset.character = state.target_id
  } else {
    $('character-art').innerHTML =
      '<div class="orbit"></div><div class="character-placeholder">✧<p>选择一位同行者</p><small>本次只围绕一个角色培养</small></div>'
    $('character-caption').innerHTML = '<small>YOUR NEXT UPGRADE</small><h2>从一个目标开始</h2><p>从账号库存出发，让已有的遗器发挥价值。</p>'
    delete document.body.dataset.character
  }
  const currentStep = state.last_result ? 'result' : state.selected_id ? 'upgrade' : state.target_id ? 'select' : 'target'
  for (const step of ['target', 'select', 'upgrade', 'result']) $(`step-${step}`).classList.toggle('active', step === currentStep)
  renderList()
  renderDetail()
  renderResult()
  renderHistory()
  renderAgent()
  renderTaskHistory()
}

function renderAgent() {
  const config = state.model_config
  const usage = state.usage.summary
  $('agent-model').textContent = `${config.model} · ${config.api_key_configured ? 'Key 已配置' : 'Key 未配置/本地模式'}`
  $('usage-input').textContent = number(usage.input_tokens)
  $('usage-output').textContent = number(usage.output_tokens)
  $('usage-calls').textContent = number(usage.calls)
  $('usage-cost').textContent = Number(usage.total_cost).toFixed(6)
  $('usage-budget').max = Math.max(config.token_budget, 1)
  $('usage-budget').value = Math.min(usage.total_tokens, config.token_budget)
  $('usage-budget-label').textContent = `${number(usage.total_tokens)} / ${number(config.token_budget)}`
  $('agent-input').disabled = busy
  $('agent-submit').disabled = busy
  const run = liveRun ?? state.last_agent
  if (!run) {
    $('agent-reply').innerHTML = '<span class="agent-mark">✦</span><div><strong>用自然语言告诉我目标</strong><p>例如：我想培养 Blade，材料比较紧，帮我看看下一件最值得强化什么。</p></div>'
    $('agent-trace').hidden = true
    return
  }
  const status = statusLabel(run.status)
  const content = run.reply ?? run.error ?? (run.status === 'running' ? '任务正在执行，下面会实时追加事件。' : '本轮没有最终回复。')
  $('agent-reply').innerHTML = `<span class="agent-mark">✦</span><div><strong>Agent ${run.status === 'running' ? '运行中' : '回复'} <small>${status}</small></strong><p>${esc(content)}</p></div>`
  $('agent-events').innerHTML = run.events.map(eventHtml).join('')
  $('agent-trace').hidden = false
}

function statusLabel(status) {
  return { running: '进行中', completed: '完成', budget_reached: '预算已达', failed: '失败', cancelled: '已取消' }[status] ?? status
}

function eventHtml(event) {
  if (event.type === 'run_started') return `<div><b>用户</b><span>${esc(event.user_input)}</span></div>`
  if (event.type === 'model_request_started') return `<div><b>模型</b><span>第 ${event.call_index} 次请求 · ${esc(event.model)}</span></div>`
  if (event.type === 'model_response_received') return `<div><b>响应</b><span>${esc(event.response_id)} · ${event.tool_call_count} 个 Tool Call${event.has_text ? ' · 含文本' : ''}</span></div>`
  if (event.type === 'usage_recorded') return `<div><b>Usage</b><span>输入 ${number(event.call.input_tokens)} · 输出 ${number(event.call.output_tokens)} · 本次费用 ${Number(event.call.cost).toFixed(6)}</span></div>`
  if (event.type === 'tool_requested') return `<div><b>Tool</b><span>${esc(event.name)}</span><code>${esc(JSON.stringify(event.arguments))}</code></div>`
  if (event.type === 'tool_progress') return `<div><b>进度</b><span>${event.stage === 'reading_state' ? '正在读取状态' : '正在等待 Rust Core / Fribbels 评价'} · ${esc(event.name)}</span></div>`
  if (event.type === 'tool_finished') return `<div><b>结果</b><span>${esc(event.name)}</span><details><summary>结构化输出</summary><pre>${esc(JSON.stringify(event.result, null, 2))}</pre></details></div>`
  if (event.type === 'decision_recorded') return `<div><b>决策</b><span>Rust Decision Engine 已返回 ${esc(event.tool_name)}</span><details><summary>查看决策数据</summary><pre>${esc(JSON.stringify(event.result, null, 2))}</pre></details></div>`
  if (event.type === 'budget_blocked') return `<div><b>预算</b><span>达到 ${number(event.used_tokens)} / ${number(event.token_budget)}，停止新请求</span></div>`
  if (event.type === 'assistant_reply') return '<div><b>回复</b><span>模型基于工具结果完成解释</span></div>'
  if (event.type === 'run_finished') return `<div><b>结束</b><span>${statusLabel(event.status)}</span></div>`
  if (event.type === 'cancelled') return `<div><b>取消</b><span>${esc(event.message)}</span></div>`
  if (event.type === 'error') return `<div><b>错误</b><span>${esc(event.message)}</span><code>${esc(event.code)}</code></div>`
  return ''
}

function renderTaskHistory() {
  const runs = [...taskState.tasks]
  if (liveRun && !runs.some((run) => run.id === liveRun.id)) runs.push(liveRun)
  $('task-count').textContent = runs.length
  if (!runs.length) {
    $('task-list').innerHTML = '<div class="history-empty">完成一次 Agent 任务后，可以在这里查看完整 Trace。</div>'
    $('task-detail').innerHTML = '<div class="history-empty">选择一个历史任务，查看用户消息、模型调用、Tool、决策、Usage 和结果。</div>'
    return
  }
  if (!selectedTaskId || !runs.some((run) => run.id === selectedTaskId)) selectedTaskId = runs[runs.length - 1].id
  $('task-list').innerHTML = [...runs].reverse().map((run, index) => `<button class="task-row ${run.id === selectedTaskId ? 'selected' : ''}" data-task="${esc(run.id)}"><span>${String(runs.length - index).padStart(2, '0')}</span><div><strong>${esc(run.user_input)}</strong><small>${new Date(run.started_at_unix_ms).toLocaleString('zh-CN')} · ${run.events.length} 个事件</small></div><b class="task-status ${run.status}">${statusLabel(run.status)}</b></button>`).join('')
  const run = runs.find((item) => item.id === selectedTaskId)
  $('task-detail').innerHTML = `<div class="task-detail-head"><div><small>${esc(run.id)}</small><h3>${esc(run.user_input)}</h3></div><span class="task-status ${run.status}">${statusLabel(run.status)}</span></div><div class="task-events">${run.events.map(eventHtml).join('')}</div>${run.reply ? `<div class="task-final"><b>最终回复</b><p>${esc(run.reply)}</p></div>` : ''}${run.error ? `<div class="task-final error"><b>终止原因</b><p>${esc(run.error)}</p></div>` : ''}`
}

function progressFor(event) {
  if (event.type === 'run_started') return '正在理解用户目标'
  if (event.type === 'model_request_started') return `正在等待模型第 ${event.call_index} 次响应`
  if (event.type === 'tool_requested') return `正在调用 ${event.name}`
  if (event.type === 'tool_progress') return event.stage === 'reading_state' ? `正在读取状态 · ${event.name}` : `正在等待 Fribbels 评价 · ${event.name}`
  if (event.type === 'tool_finished') return `工具执行完成 · ${event.name}`
  if (event.type === 'assistant_reply') return '正在生成最终回复'
  if (event.type === 'cancelled') return '任务已取消，正在保存已完成轨迹'
  if (event.type === 'error') return '任务结束，正在保存错误轨迹'
  return progressLabel
}

function receiveTrace(event) {
  if (!liveRun || liveRun.id !== event.run_id) {
    liveRun = { id: event.run_id, status: 'running', started_at_unix_ms: event.recorded_at_unix_ms, finished_at_unix_ms: null, user_input: event.user_input ?? '', reply: null, error: null, events: [] }
  }
  if (!liveRun.events.some((item) => item.sequence === event.sequence)) liveRun.events.push(event)
  if (event.type === 'run_started') liveRun.user_input = event.user_input
  if (event.type === 'assistant_reply') liveRun.reply = event.content
  if (event.type === 'run_finished') liveRun.status = event.status
  if (event.type === 'cancelled') { liveRun.status = 'cancelled'; liveRun.error = event.message }
  if (event.type === 'error') { liveRun.status = 'failed'; liveRun.error = event.message }
  progressLabel = progressFor(event)
  if (!$('progress').hidden) $('progress-text').textContent = `${progressLabel}…`
  selectedTaskId = liveRun.id
  renderAgent()
  renderTaskHistory()
}

async function refreshTasks() {
  taskState = await api.tasks()
  liveRun = taskState.active
  renderTaskHistory()
}
function renderList() {
  $('rank-count').textContent = state.recommendations.length
  $('tab-ranked').setAttribute('aria-selected', tab === 'ranked')
  $('tab-all').setAttribute('aria-selected', tab === 'all')
  $('list-caption').textContent = tab === 'ranked' ? '先按游戏静态套装推荐筛选，再按强化价值排序' : '包含未推荐、暂缓及受保护的遗器'
  const scores = new Map(state.recommendations.map((r) => [r.relic_id, r]))
  const inventory = new Map(state.inventory.map((r) => [r.id, r]))
  // Ordering comes from the API. The browser only joins display data by ID.
  const rows = tab === 'ranked' ? state.recommendations.map((r) => inventory.get(r.relic_id)) : state.inventory
  $('relic-list').innerHTML = rows.length
    ? rows.map((r, i) => {
      const score = scores.get(r.id)
      const badge = r.blocked ? (r.decision ?? '已保护') : r.decision === 'Hold' ? '恢复观察' : tab === 'ranked' && i === 0 ? '首选候选' : slots[r.slot]
      return `<button class="relic-card ${state.selected_id === r.id ? 'selected' : ''}" data-relic="${r.id}" ${busy || r.blocked ? 'disabled' : ''} title="${
        esc(r.blocked?.message ?? score?.reason ?? '选择这件遗器')
      }" aria-pressed="${state.selected_id === r.id}"><div class="relic-thumb">${
        img(assets.relic(r), '', slots[r.slot])
      }<span>+${r.level}</span></div><div class="relic-info"><div class="relic-card-top"><span class="card-badge ${badge === '首选候选' ? 'gold' : ''}">${
        esc(badge)
      }</span><small>#${r.id}</small></div><h3>${sets[r.set_id] ?? esc(r.set_id)}</h3>${
        r.set_match === 'recommended'
          ? '<small class="static-fit recommended">游戏静态推荐套装</small>'
          : r.set_match === 'not_recommended'
            ? '<small class="static-fit excluded">未列入该角色静态推荐</small>'
            : ''
      }<div class="relic-main">${img(assets.stat(r.main_stat))}${
        stats[r.main_stat]
      }</div><div class="mini-stats">${Object.entries(r.substats).map(([s, n]) => `<span>${stats[s]} ${value(s, n)}</span>`).join('')}</div>${
        score
          ? `<div class="card-score"><span>当前 <b>${number(score.current_score)}</b></span><span>平均潜力 <b>${number(score.projected_score)}</b></span></div>`
          : `<small class="blocked-note">${esc(r.blocked?.message ?? '可手动选择并观察')}</small>`
      }</div></button>`
    }).join('')
    : `<div class="empty"><span>◇</span><p>${state.target_id ? '暂无可推荐的候选' : '推荐将在这里出现'}</p><small>${
      state.target_id ? '查看全部库存，或重置本次试用' : '先在左侧选择刃或希儿'
    }</small></div>`
}
function renderDetail() {
  const r = state.inventory.find((r) => r.id === state.selected_id)
  if (!r) {
    $('detail').innerHTML = `<div class="detail-empty"><div class="empty-orbit">✦</div><h2>${
      state.target_id ? '本轮暂无选中遗器' : '准备好下一次强化'
    }</h2><p>${
      state.target_id
        ? '可从全部库存中恢复暂缓的候选，<br>或重置 Demo 开始新的观察。'
        : '选择目标后，我们会推荐一件候选遗器。<br>你也可以从库存中选择想继续观察的遗器。'
    }</p><div class="empty-tags"><span>评分与潜力</span><span>逐次观察</span><span>随时换一件</span></div></div>`
    return
  }
  const ev = state.selected_evaluation
  $('detail').innerHTML = `<div class="relic-display"><div class="relic-halo"></div>${
    img(assets.relic(r), 'large-relic', sets[r.set_id])
  }<div class="relic-title"><div class="stars">★★★★★ <span>+${r.level}</span></div><h2>${sets[r.set_id]}</h2><p>${
    slots[r.slot]
  } <span> / </span> #${r.id}</p></div></div><div class="detail-body"><div class="main-stat">${img(assets.stat(r.main_stat))}<span>主属性</span><strong>${
    stats[r.main_stat]
  }</strong></div><div class="substats">${
    Object.entries(r.substats).map(([s, n]) => `<div>${img(assets.stat(s))}<span>${stats[s]}</span><b>${value(s, n)}</b></div>`).join('')
  }</div><div class="scores"><div><small>当前评分</small><strong>${
    number(ev.current_score)
  }</strong></div><span class="score-arrow">⟶</span><div><small>平均满级潜力</small><strong>${
    number(ev.projected_score)
  }</strong></div></div><form id="upgrade-form"><div class="form-heading"><h3>录入一次强化结果</h3><span>每次 +3 · 消耗 1 步</span></div><div class="form-fields"><label>本次变化的副属性<select id="stat" name="stat" required ${
    busy ? 'disabled' : ''
  }><option value="">请选择属性</option>${
    inputs.map((s) => `<option value="${s}">${stats[s]}</option>`).join('')
  }</select></label><label>增加了多少<input id="increase" name="increase" type="number" inputmode="decimal" min="0.00001" step="any" placeholder="例如 3.24" required ${
    busy ? 'disabled' : ''
  }></label></div><p class="field-hint">填写本次增量，不是强化后的总值；百分比填百分点。</p><button type="submit" id="submit-upgrade" class="primary-button" ${
    busy ? 'disabled' : ''
  }>${busy ? '正在评价…' : '记录强化结果'} <span>↗</span></button></form><details class="metric-details"><summary>查看本件的评价数据与推荐依据</summary><p>${
    esc(state.recommendations.find((s) => s.relic_id === r.id)?.reason ?? '这件遗器由你手动选择，是否继续由下次观察后的评价决定。')
  }</p>${
    ev.details
      ? `<p>满级潜力：最低 ${number(ev.details.relic.worst)} / 平均 ${number(ev.details.relic.average)} / 最高 ${
        number(ev.details.relic.best)
      }</p><p>参考配装替换后：HP ${number(ev.details.candidate_build.panel.hp)} · ATK ${number(ev.details.candidate_build.panel.atk)} · SPD ${
        number(ev.details.candidate_build.panel.speed)
      }</p><p>简化普攻 ${number(ev.details.candidate_build.basic_damage)}；参考 ${
        number(ev.details.reference_build.basic_damage)
      }。这不代表完整角色输出。</p><p>参考六件 ${esc(ev.details.reference.relic_ids.join('、'))}；补齐部位 ${
        ev.details.reference.assumed_slots.map((s) => slots[s]).join('、') || '无'
      }。</p>`
      : '<p>当前为 Mock 对照模式。</p>'
  }</details></div>`
  $('upgrade-form').addEventListener('submit', (e) => {
    e.preventDefault()
    const stat = $('stat').value, increase = Number($('increase').value)
    perform({ action: 'upgrade', relic_id: r.id, expected_level: r.level, stat, increase })
  })
}
function renderResult() {
  const r = state.last_result
  $('result-panel').hidden = !r
  if (!r) return
  $('result-panel').className = `result-panel result-${r.decision.toLowerCase()}`
  $('result-panel').innerHTML = `<div class="result-symbol">${
    { Continue: '↗', Hold: 'Ⅱ', Stop: '■' }[r.decision]
  }</div><div class="result-copy"><small>最近一次观察 · 遗器 #${esc(r.relic_id)}</small><h2>${labels[r.decision]} <span>${r.decision}</span></h2><p>${
    esc(r.reason)
  }</p>${
    state.selected_id ? `<small>现在选中 #${esc(state.selected_id)}，可在强化观察区继续。</small>` : '<small>当前没有选中遗器，可以查看库存或重置试用。</small>'
  }</div>`
}
function renderHistory() {
  $('history-count').textContent = state.history.length
  $('history').className = state.history.length ? 'history-list' : 'history-empty'
  $('history').innerHTML = state.history.length
    ? [...state.history].reverse().map((r, i) =>
      `<div class="history-row"><span class="history-index">${String(state.history.length - i).padStart(2, '0')}</span><span>${names[r.character_id]} <small>#${
        esc(r.relic_id)
      }</small></span><span>+${r.before_level} <i>→</i> +${r.after_level}</span><span>${stats[r.stat]} +${
        value(r.stat, r.increase)
      }</span><span class="decision-pill ${r.decision.toLowerCase()}">${r.decision}</span><details><summary>原因</summary><p>${
        esc(r.reason)
      }</p></details></div>`
    ).join('')
    : '每一次观察都会留在这里。重新加载页面可继续本次试用。'
}
async function perform(action) {
  if (busy) return
  const draft = $('upgrade-form') ? { stat: $('stat').value, increase: $('increase').value } : null
  busy = true
  $('error').hidden = true
  $('progress').hidden = false
  progressLabel = '正在重新评价遗器'
  $('progress-text').textContent = `${progressLabel}…`
  render()
  timer = setInterval(async () => {
    try {
      const p = await api.request('progress')
      $('progress-text').textContent = `${progressLabel} · 已等待 ${p.seconds} 秒`
    } catch { /* The action reports connectivity errors. */ }
  }, 700)
  let failed = false
  try {
    state = await api.action(action, state.revision)
    if (action.action === 'target') tab = 'ranked'
    if (action.action === 'reset') tab = 'ranked'
  } catch (e) {
    failed = true
    $('error').textContent = e.message
    $('error').hidden = false
    try {
      state = await api.request('state')
    } catch { /* Keep the last visible snapshot. */ }
  } finally {
    clearInterval(timer)
    busy = false
    $('progress').hidden = true
    render()
    if (failed && draft && $('stat')) {
      $('stat').value = draft.stat
      $('increase').value = draft.increase
    }
    if (!failed && action.action === 'upgrade') $('result-panel').scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  }
}

async function performAgent(input) {
  if (busy) return
  busy = true
  progressLabel = 'Agent 正在理解目标并调用工具'
  $('error').hidden = true
  $('progress').hidden = false
  $('progress-text').textContent = `${progressLabel}…`
  render()
  timer = setInterval(async () => {
    try {
      const p = await api.request('progress')
      $('progress-text').textContent = `${progressLabel} · 已等待 ${p.seconds} 秒`
    } catch { /* The Agent request reports connection errors. */ }
  }, 700)
  let failed = false
  try {
    state = await api.agent(input, state.revision)
    tab = 'ranked'
    $('agent-input').value = ''
  } catch (e) {
    failed = true
    $('error').textContent = e.message
    $('error').hidden = false
    try {
      state = await api.request('state')
    } catch { /* Keep the last visible snapshot. */ }
  } finally {
    clearInterval(timer)
    busy = false
    $('progress').hidden = true
    try { await refreshTasks() } catch { /* State response still contains the latest task. */ }
    render()
    if (failed) $('agent-input').value = input
  }
}
$('characters').addEventListener('click', (e) => {
  const b = e.target.closest('[data-target]')
  if (b) perform({ action: 'target', character_id: b.dataset.target })
})
$('relic-list').addEventListener('click', (e) => {
  const b = e.target.closest('[data-relic]')
  if (b && !b.disabled) perform({ action: 'select', relic_id: b.dataset.relic })
})
$('tab-ranked').onclick = () => {
  if (!state) return
  tab = 'ranked'
  renderList()
}
$('tab-all').onclick = () => {
  if (!state) return
  tab = 'all'
  renderList()
}
$('reset').onclick = () => $('reset-dialog').showModal()
$('reset-no').onclick = () => $('reset-dialog').close()
$('reset-yes').onclick = () => {
  $('reset-dialog').close()
  perform({ action: 'reset' })
}
$('guide-button').onclick = () => $('guide-dialog').showModal()
$('guide-close').onclick = () => $('guide-dialog').close()
$('agent-form').onsubmit = (event) => {
  event.preventDefault()
  performAgent($('agent-input').value)
}
$('task-list').onclick = (event) => {
  const row = event.target.closest('[data-task]')
  if (row) {
    selectedTaskId = row.dataset.task
    renderTaskHistory()
  }
}
$('save-session').onclick = async () => {
  try {
    const saved = await api.exportSession()
    const blob = new Blob([`${JSON.stringify(saved, null, 2)}\n`], { type: 'application/json' })
    const link = document.createElement('a')
    link.href = URL.createObjectURL(blob)
    link.download = `hsr-agent-session-${new Date().toISOString().replace(/[:.]/g, '-')}.json`
    link.click()
    URL.revokeObjectURL(link.href)
  } catch (e) {
    $('error').textContent = e.message
    $('error').hidden = false
  }
}
$('load-session').onclick = () => $('session-file').click()
$('session-file').onchange = async () => {
  const file = $('session-file').files[0]
  $('session-file').value = ''
  if (!file) return
  try {
    const saved = JSON.parse(await file.text())
    state = await api.importSession(saved, state.revision)
    await refreshTasks()
    selectedTaskId = taskState.tasks.at(-1)?.id ?? null
    tab = 'ranked'
    $('error').hidden = true
    render()
  } catch (e) {
    $('error').textContent = e instanceof SyntaxError ? '会话文件不是有效 JSON。' : e.message
    $('error').hidden = false
  }
}
$('model-settings').onclick = () => {
  const config = state.model_config
  $('config-endpoint').value = config.endpoint
  $('config-key').value = ''
  $('config-clear-key').checked = false
  $('config-model').value = config.model
  $('config-context').value = config.context_length
  $('config-reasoning').value = config.reasoning_mode
  $('config-input-price').value = config.input_price_per_million
  $('config-output-price').value = config.output_price_per_million
  $('config-token-budget').value = config.token_budget
  $('model-error').hidden = true
  $('model-dialog').showModal()
}
$('model-cancel').onclick = () => $('model-dialog').close()
$('model-form').onsubmit = async (event) => {
  event.preventDefault()
  $('model-error').hidden = true
  const patch = {
    endpoint: $('config-endpoint').value.trim(),
    api_key: $('config-key').value || null,
    clear_api_key: $('config-clear-key').checked,
    model: $('config-model').value.trim(),
    context_length: Number($('config-context').value),
    reasoning_mode: $('config-reasoning').value,
    input_price_per_million: Number($('config-input-price').value),
    output_price_per_million: Number($('config-output-price').value),
    token_budget: Number($('config-token-budget').value),
  }
  try {
    state = await api.modelConfig(patch, state.revision)
    $('config-key').value = ''
    $('model-dialog').close()
    render()
  } catch (e) {
    $('model-error').textContent = e.message
    $('model-error').hidden = false
  }
}
$('cancel').onclick = async () => {
  try {
    await api.request('cancel', {})
    $('progress-text').textContent = '正在取消…'
  } catch (e) {
    $('error').textContent = e.message
    $('error').hidden = false
  }
}
try {
  ;[assets, state] = await Promise.all([AssetProvider.load(), api.start()])
  taskState = await api.tasks()
  eventSource = api.events(receiveTrace)
  render()
} catch (e) {
  $('error').textContent = `暂时无法连接培养终端：${e.message} 请确认服务已启动后刷新页面。`
  $('error').hidden = false
}
