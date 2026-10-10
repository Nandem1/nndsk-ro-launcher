import assert from 'node:assert/strict'
import { chromium } from 'playwright'
import { auditFit } from './design-fit.mjs'

const browser = await chromium.launch({
  executablePath: process.env.RO_DESIGN_CHROMIUM ?? '/usr/bin/chromium',
  headless: true,
})
try {
  const page = await browser.newPage({ viewport: { width: 800, height: 600 } })
  const reset = () =>
    page.setContent(`
    <style>body{margin:20px}section{position:relative;width:300px;height:180px;margin-bottom:20px;background:#151618}h2{height:24px;margin:0}.body{height:100px;overflow:auto}.row{height:40px}button{height:26px}</style>
    <section class="rounded-panel"><h2>First</h2><div class="body" data-design-panel-scroll><div id="widget" class="row"><button>Complete control</button></div><div class="row">Second row</div><div class="row">Third row</div></div></section>
    <section class="rounded-panel"><h2>Second</h2><button>Another control</button></section>
  `)
  await reset()
  assert.deepEqual((await auditFit(page)).failures, [])
  const cases = [
    [
      'partial-content',
      () => {
        const widget = document.querySelector('#widget')
        widget.style.height = '10px'
        widget.style.overflow = 'hidden'
      },
    ],
    [
      'panel-overlap',
      () => {
        document.querySelectorAll('section')[1].style.top = '-50px'
      },
    ],
    [
      'panel-escape',
      () => {
        document
          .querySelectorAll('section')[1]
          .querySelector('button').style.marginLeft = '310px'
      },
    ],
    [
      'nested-scroll',
      () => {
        const widget = document.querySelector('#widget')
        widget.style.overflow = 'auto'
        widget.style.height = '20px'
      },
    ],
    [
      'select-anchor',
      () => {
        const trigger = document.querySelector('button')
        trigger.setAttribute('aria-controls', 'bad-list')
        const list = document.createElement('ul')
        list.id = 'bad-list'
        list.setAttribute('role', 'listbox')
        list.style.cssText =
          'position:fixed;left:400px;top:400px;width:200px;height:80px'
        list.textContent = 'Detached option'
        document.body.append(list)
      },
    ],
  ]
  for (const [kind, mutate] of cases) {
    await reset()
    await page.evaluate(mutate)
    const result = await auditFit(page)
    assert.ok(
      result.failures.some((failure) => failure.kind === kind),
      `${kind}: ${JSON.stringify(result.failures)}`,
    )
    console.log(`${kind}: invalid fixture detected`)
  }
} finally {
  await browser.close()
}
