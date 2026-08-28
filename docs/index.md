---
# https://vitepress.dev/reference/default-theme-home-page
layout: home

hero:
  name: "handl"
  text: "Platform Docs"
  tagline: Build your toolchain one plugin at a time.
  actions:
    - theme: brand
      text: Get started
      link: /tutorials/getting-started
    - theme: brand
      text: Build a host
      link: /tutorials/build-a-host
    - theme: alt
      text: How-to guides
      link: /how-to/
    - theme: alt
      text: Explanations
      link: /explanation/

features:
  - title: Universal plugins
    details: One Plugin trait unifies libraries, targets, operations, rules, and options.
    link: /explanation/composable-host
    linkText: Learn more
  - title: Composable host
    details: The Host is an empty composer that loads kits, resolves dependencies, and runs capabilities.
    link: /tutorials/build-a-host
    linkText: Build one
  - title: Kits and manifests
    details: Curated, version-pinned kits make language toolchains reproducible and swappable.
    link: /explanation/kits-and-manifests
    linkText: Explore kits
  - title: Type-safe rules
    details: contribute::<P>(rule) is checked at compile time, so a Format rule cannot reach a Linter.
    link: /explanation/plugins-rules-operations
    linkText: See how it works
  - title: Plugin-defined options
    details: Options are declared by the plugins that own them and resolved from kit, host, and user config.
    link: /explanation/options-and-configuration
    linkText: Read about options
  - title: Multiple targets
    details: Targets are plugins, so the same language can compile to native, wasm, JS, or an interpreter.
    link: /explanation/platform-architecture
    linkText: View architecture
---

<script setup>
import { onMounted } from 'vue'

onMounted(() => {
  const hero = document.querySelector('.VPHero')
  if (!hero) return

  const canvas = document.createElement('canvas')
  canvas.style.position = 'absolute'
  canvas.style.inset = '0'
  canvas.style.width = '100%'
  canvas.style.height = '100%'
  canvas.style.fontSize = '12px'
  canvas.style.fontFamily = 'var(--vp-font-family-mono)'
  canvas.style.zIndex = '0'
  hero.style.position = 'relative'
  hero.insertBefore(canvas, hero.firstChild)

  const ctx = canvas.getContext('2d')

  const settings = {
    maxDepth: 4,
    statementCount: { min: 60, max: 140 },
    branchChance: 0.35,
    indentSize: 24,
    lineHeight: 18,
    space: 6,
    alpha: 0.18
  }

  const randInt = (min, max) => min + Math.floor(Math.random() * (max - min + 1))
  const pick = (arr) => arr[Math.floor(Math.random() * arr.length)]
  const chance = (p) => Math.random() < p

  const vars = [
    'x', 'y', 'z', 'u', 'v', 'w', 'a', 'b', 'c', 'd', 'e',
    'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't'
  ]
  const caps = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
    'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z'
  ]
  const types = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'M', 'T', 'K', 'R', 'S', 'P', 'Q',
    'Int', 'Float', 'Bool', 'String', 'Unit', 'Type', 'Kind', 'Constraint',
    'Eff', 'IO', 'List', 'Maybe', 'Option', 'Result', 'Either', 'Task',
    'Stream', 'Handler', 'Cont', 'State', 'Reader', 'Writer', 'Promise', 'Future',
    'Category', 'Functor', 'Monad', 'Arrow', 'Profunctor', 'Bifunctor',
    'Traversable', 'Foldable'
  ]
  const ops = [
    '->', '=>', '::', ':', '~>', '>=>', '<=<', '<<<', '>>>',
    '**', '&&', '||', '++', '>>=', '<$>', '<*>',
    '=', '!=', '==', '<', '>', '<=', '>=', '+', '-', '*', '/', '%',
    '&', '|', '!', '?', '@', '#', '.'
  ]
  const keywords = [
    'let', 'in', 'where', 'fun', 'val', 'op', 'handle', 'with', 'perform',
    'resume', 'signal', 'do', 'return', 'yield', 'match', 'rec', 'fix', 'lam',
    'forall', 'exists', 'arr', 'first', 'second', 'loop', 'delay', 'pre',
    'feedback', 'run', 'eval', 'compose', 'id', 'lift', 'map', 'fold', 'unfold',
    'coalg', 'alg', 'free', 'cofree', 'mu', 'nu', 'pure', 'bind', 'fmap', 'ap',
    'join', 'sequence', 'traverse', 'handler', 'operation', 'request', 'continue',
    'abort', 'raise', 'catch', 'subst', 'apply', 'curry', 'uncurry', 'swap', 'dup',
    'fst', 'snd', 'unit', 'void', 'nil', 'cons', 'snoc', 'head', 'tail', 'init',
    'last', 'take', 'drop', 'filter', 'zip', 'unzip', 'foldl', 'foldr', 'scanl',
    'scanr', 'iterate', 'repeat', 'replicate', 'concat', 'append', 'reverse', 'sort'
  ]
  const literals = [
    '0', '1', '2', '3', '42', '123', '"a"', '"b"', '"c"',
    'true', 'false', '()', '[]'
  ]
  const delim = ['(', ')', '{', '}', '[', ']', '<', '>', '|']

  const varr = () => pick(vars)
  const cap = () => pick(caps)
  const ty = () => pick(types)
  const lit = () => pick(literals)
  const op = () => pick(ops)
  const kw = (w) => w
  const p = (s) => s

  function repeat(fn, min, max) {
    const out = []
    for (let i = 0, n = randInt(min, max); i < n; i++) out.push(...fn())
    return out
  }

  function program() {
    return repeat(stmt, settings.statementCount.min, settings.statementCount.max)
  }

  function stmt(depth = 0) {
    if (depth >= settings.maxDepth) return [...expr(depth + 1), p(';')]
    const roll = Math.random()
    if (roll < 0.15) return [kw('fun'), cap(), p('('), ...params(depth + 1), p(')'), p(':'), ...type(depth + 1), p('->'), ...expr(depth + 1), p(';')]
    if (roll < 0.30) return [kw('val'), varr(), p(':'), ...type(depth + 1), p('='), ...expr(depth + 1), p(';')]
    if (roll < 0.42) return [kw('op'), cap(), p('('), ...params(depth + 1), p(')'), p(':'), ...type(depth + 1), p('->'), ...type(depth + 1), p(';')]
    if (roll < 0.54) return [kw('handle'), ...expr(depth + 1), kw('with'), p('{'), ...arms(depth + 1), p('}')]
    if (roll < 0.66) return [kw('let'), varr(), p('='), ...expr(depth + 1), kw('in'), ...expr(depth + 1)]
    if (roll < 0.78) return [kw('rec'), varr(), p('.'), ...expr(depth + 1), p(';')]
    if (roll < 0.88) return [kw('match'), ...expr(depth + 1), kw('with'), p('{'), ...arms(depth + 1), p('}')]
    return [...expr(depth + 1), p('::'), ...type(depth + 1), p(';')]
  }

  function params(depth) {
    if (depth >= settings.maxDepth) return [varr(), p(':'), ...type(depth + 1)]
    return repeat(() => [varr(), p(':'), ...type(depth + 1)], 0, 3).reduce((a, b, i) => (i ? a.concat(p(','), ...b) : a.concat(...b)), [])
  }

  function arms(depth) {
    if (depth >= settings.maxDepth) return [...expr(depth + 1), p('->'), ...expr(depth + 1)]
    return repeat(() => [...expr(depth + 1), p('->'), ...expr(depth + 1)], 1, 4).flatMap((g, i) => (i ? [p('|'), ...g] : g))
  }

  function doStmts(depth) {
    if (depth >= settings.maxDepth) return [varr(), p('<-'), ...expr(depth + 1)]
    return repeat(() => [varr(), p('<-'), ...expr(depth + 1)], 1, 4).flatMap((g, i) => (i ? [p(';'), ...g] : g))
  }

  function expr(depth) {
    if (depth >= settings.maxDepth) return [pick([cap, varr, lit])()]
    const roll = Math.random()
    if (roll < 0.18) return [pick([cap, varr, lit])()]
    if (roll < 0.32) return [cap(), p('('), ...args(depth + 1), p(')')]
    if (roll < 0.48) return [...expr(depth + 1), op(), ...expr(depth + 1)]
    if (roll < 0.60) return [p('('), ...expr(depth + 1), p(')'), p('->'), ...expr(depth + 1)]
    if (roll < 0.72) return [kw('lam'), varr(), p('=>'), ...expr(depth + 1)]
    if (roll < 0.82) return [kw('do'), p('{'), ...doStmts(depth + 1), p('}')]
    if (roll < 0.90) return [p('<'), ...expr(depth + 1), p(','), ...expr(depth + 1), p('>')]
    return [p('['), ...expr(depth + 1), p(']'), p('->'), ...expr(depth + 1)]
  }

  function args(depth) {
    if (depth >= settings.maxDepth) return [expr(depth + 1)]
    return repeat(() => expr(depth + 1), 0, 4).flatMap((g, i) => (i ? [p(','), ...g] : g))
  }

  function type(depth) {
    if (depth >= settings.maxDepth) return [ty()]
    const roll = Math.random()
    if (roll < 0.35) return [ty()]
    if (roll < 0.55) return [...type(depth + 1), p('->'), ...type(depth + 1)]
    if (roll < 0.75) return [...type(depth + 1), p('*'), ...type(depth + 1)]
    if (roll < 0.88) return [p('('), ...type(depth + 1), p(')'), p('->'), ...type(depth + 1)]
    return [p('Eff'), p('<'), ...type(depth + 1), p('>')]
  }

  function tokenStream() {
    return program()
  }

  function draw() {
    const rect = hero.getBoundingClientRect()
    const dpr = window.devicePixelRatio || 1
    canvas.width = Math.floor(rect.width * dpr)
    canvas.height = Math.floor(rect.height * dpr)
    canvas.style.width = rect.width + 'px'
    canvas.style.height = rect.height + 'px'
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, rect.width, rect.height)

    canvas.style.color = 'var(--primary)'
    const style = getComputedStyle(canvas)
    ctx.font = style.fontSize + ' ' + style.fontFamily
    ctx.fillStyle = style.color
    ctx.globalAlpha = settings.alpha
    ctx.textBaseline = 'top'

    let queue = []
    function nextToken() {
      while (queue.length === 0) queue = stmt(0)
      return queue.shift()
    }

    let x = 0
    let y = 0
    let indent = 0

    while (y < rect.height) {
      const t = nextToken()
      if (t === '\n') {
        x = indent * settings.indentSize
        y += settings.lineHeight
        continue
      }
      if (t === '{') indent++
      if (t === '}') indent = Math.max(0, indent - 1)

      const w = ctx.measureText(t).width
      if (x + w > rect.width) {
        x = indent * settings.indentSize
        y += settings.lineHeight
        if (y >= rect.height) break
      }
      ctx.fillText(t, x, y)
      x += w + settings.space
    }
  }

  draw()
  window.addEventListener('resize', draw)
})
</script>

## Documentation structure

This documentation follows the [Diátaxis](https://diataxis.fr/) framework, which separates documentation into four purposes:

- **Tutorials** — step-by-step lessons for learning handl and the platform.
- **How-to guides** — focused recipes for solving specific problems.
- **Explanations** — background and conceptual understanding.
- **Reference** — generated facts about functions, types, and APIs.
