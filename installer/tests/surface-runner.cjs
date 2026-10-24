// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Exercise the exact exported document in a hidden, offscreen renderer. */
const { app, BrowserWindow, ipcMain } = require('electron')
const fs = require('node:fs/promises')
const path = require('node:path')
const assert = require('node:assert/strict')
const [source, output, scale] = process.argv.slice(2)
app.disableHardwareAcceleration()
app.commandLine.appendSwitch('force-device-scale-factor', scale)
app.commandLine.appendSwitch('force-prefers-reduced-motion')
app.setPath('userData', path.join(output, 'surface-profile'))
let window
const commands = []
const inspections = []
let install
let inspection
let opening
let failure
let copy
let maintenance = false
const iconMeasurements = []
async function evaluate(code) { return window.webContents.executeJavaScript(code) }
async function elapsed(milliseconds) {
  await evaluate(`window.qualificationClock ??= performance.now.bind(performance);
    Object.defineProperty(performance, 'now', { configurable: true,
      value: () => window.qualificationClock() + ${milliseconds} });`)
}
async function deliver(message) {
  await evaluate(`window.setupReceive(${JSON.stringify(message)})`)
}
async function until(expression) {
  await evaluate(`new Promise((resolve, reject) => {
    if (${expression}) return resolve();
    const timer = setTimeout(() => { observer.disconnect(); reject(new Error('Surface condition timed out')); }, 5000);
    const observer = new MutationObserver(() => {
      if (${expression}) { clearTimeout(timer); observer.disconnect(); resolve(); }
    });
    observer.observe(document.body, {subtree:true,attributes:true,childList:true});
  })`)
}
async function paint() { await evaluate('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))') }
/** Preserve the Fluent control metrics and align actions when card descriptions wrap. */
async function verifyPageGeometry(page) {
  const problems = await evaluate(`(() => {
    const problems = [];
    const step = document.getElementById('step');
    const range = document.createRange();
    range.selectNodeContents(step);
    const label = range.getBoundingClientRect();
    const journey = document.querySelector('.journey').getBoundingClientRect();
    const brand = document.getElementById('brand').getBoundingClientRect();
    const wordmark = document.querySelector('.wordmark').getBoundingClientRect();
    const bar = document.getElementById('journey').getBoundingClientRect();
    if (range.getClientRects().length !== 1) problems.push('Step label must stay on one line');
    if (label.left < journey.left - 0.5 || label.right > journey.right + 0.5 ||
        journey.left < wordmark.right + 24 - 0.5 || journey.right > brand.right - 52 + 0.5 ||
        Math.abs(wordmark.width - 270) > 0.5) problems.push('Step label must fit beside the full wordmark');
    if (Math.abs(bar.width - journey.width) > 0.5 || label.bottom > bar.top)
      problems.push('Step progress must align below its full label');
    for (const element of document.querySelectorAll('button, input:not([type=radio], [type=checkbox]), select')) {
      if (!element.checkVisibility() || element.classList.contains('caption')) continue;
      if (getComputedStyle(element).fontSize !== '14px') problems.push('Control text must be 14px: ' + element.id);
      if (element.getBoundingClientRect().height < 32) problems.push('Control too short: ' + element.id);
    }
    for (const element of document.querySelectorAll('.language-page .hero p, .location-page .hero p')) {
      if (element.checkVisibility() && getComputedStyle(element).fontSize !== '14px') problems.push('Introductory body text must be 14px');
    }
    for (const field of document.querySelectorAll('.field')) {
      if (!field.checkVisibility()) continue;
      const input = field.querySelector('input').getBoundingClientRect();
      const button = field.querySelector('button').getBoundingClientRect();
      if (Math.abs((input.top + input.bottom - button.top - button.bottom) / 2) > 0.5) problems.push('Browse alignment');
    }
    const choices = [...document.querySelectorAll('.choice')].filter(element => element.checkVisibility());
    if (choices.length === 2 && choices[0].offsetTop === choices[1].offsetTop) {
      const buttons = choices.map(choice => choice.querySelector('.choice-selection').getBoundingClientRect());
      if (Math.abs(buttons[0].bottom - buttons[1].bottom) > 0.5) problems.push('Choice actions must share a baseline');
    }
    return problems;
  })()`)
  assert.deepEqual(problems, [], `${page}: page geometry`)
}
/** A page uses each decorative icon once; field affordances may repeat with their controls. */
async function verifyContentHierarchy(page) {
  const result = await evaluate(`(() => {
    const page = document.querySelector('.page:not([hidden])');
    const icons = [...page.querySelectorAll('.hero-icon svg, .choice-icon svg, .completion-panel svg')]
      .filter(icon => icon.checkVisibility()).map(icon => icon.innerHTML);
    const headings = [...page.querySelectorAll('h1')].filter(element => element.checkVisibility());
    const title = headings[0].textContent.trim();
    const duplicateTitle = [...page.querySelectorAll('p, small, dd')].some(element =>
      element.checkVisibility() && element.textContent.trim() === title);
    return { headingCount: headings.length, duplicateIcons: icons.length - new Set(icons).size, duplicateTitle };
  })()`)
  assert.deepEqual(result, { headingCount: 1, duplicateIcons: 0, duplicateTitle: false }, `${page}: content hierarchy`)
}
/** Check the icon viewport and the actual white glyph, including upstream inline SVG sizing. */
async function verifyIcons(page) {
  const icons = await evaluate(`Array.from(document.querySelectorAll(
    '.hero-icon, .choice-icon, .caption, .select-control > span'
  )).filter(element => element.checkVisibility()).map(element => {
    const svg = [...element.querySelectorAll('svg')].find(icon => icon.checkVisibility());
    const badge = element.getBoundingClientRect();
    const viewport = svg.getBoundingClientRect();
    const bounds = svg.getBBox();
    const glyphCenter = new DOMPoint(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)
      .matrixTransform(svg.getScreenCTM());
    const size = element.matches('.choice-icon') ? 20 :
      element.matches('.caption, .select-control > span') ? 10 :
      element.closest('.language-page, .location-page') ? 24 : 22;
    const stage = element.closest('main')?.getBoundingClientRect();
    return { kind: element.className, size,
      fullyVisible: !stage || (badge.top >= stage.top && badge.bottom <= stage.bottom),
      glyphCenter: { x: glyphCenter.x, y: glyphCenter.y },
      badge: { x: badge.x, y: badge.y, width: badge.width, height: badge.height },
      viewport: { x: viewport.x, y: viewport.y, width: viewport.width, height: viewport.height } };
  })`)
  const screenshot = await window.webContents.capturePage()
  // innerWidth rounds fractional CSS pixels; dividing by it shifts raster checks near the edge.
  const viewport = await evaluate('({ ratio: devicePixelRatio, width: document.documentElement.getBoundingClientRect().width })')
  const ratio = viewport.ratio
  assert.ok(Math.abs(screenshot.getSize().width - viewport.width * ratio) <= 1,
    `${page}: capture pixels must match the renderer scale`)
  for (const [index, icon] of icons.entries()) {
    const { badge, viewport, size } = icon
    const description = `${page} ${icon.kind} #${index}`
    assert.equal(viewport.width, size, `${description}: icon width`)
    assert.equal(viewport.height, size, `${description}: icon height`)
    assert.ok(Math.abs(viewport.x + viewport.width / 2 - badge.x - badge.width / 2) <= 0.5,
      `${description}: horizontal viewport centering`)
    assert.ok(Math.abs(viewport.y + viewport.height / 2 - badge.y - badge.height / 2) <= 0.5,
      `${description}: vertical viewport centering`)
    // Preserve the original Fluent artwork's optical offsets inside its centered viewport.
    assert.ok(Math.abs(icon.glyphCenter.x - badge.x - badge.width / 2) <= 1 &&
      Math.abs(icon.glyphCenter.y - badge.y - badge.height / 2) <= 1,
      `${description}: artwork bounds centering`)
    const crop = { x: Math.max(0, Math.floor(badge.x * ratio)), y: Math.floor(badge.y * ratio),
      width: Math.min(screenshot.getSize().width, Math.ceil((badge.x + badge.width) * ratio)) -
        Math.max(0, Math.floor(badge.x * ratio)),
      height: Math.ceil((badge.y + badge.height) * ratio) - Math.floor(badge.y * ratio) }
    // Compact pages may scroll; fully visible badges are checked in pixels as well as layout.
    if (!icon.fullyVisible || crop.y < 0 || crop.y + crop.height > screenshot.getSize().height) continue
    const pixels = screenshot.crop(crop).toBitmap()
    assert.equal(pixels.length, crop.width * crop.height * 4, `${description}: crop pixel dimensions`)
    const ink = { left: Infinity, top: Infinity, right: -Infinity, bottom: -Infinity }
    for (let y = 0; y < crop.height; y++) {
      for (let x = 0; x < crop.width; x++) {
        const offset = (y * crop.width + x) * 4
        // Include antialiased stroke tips while excluding the dark badge and pink border.
        if (pixels[offset] < 90 || pixels[offset + 1] < 90 || pixels[offset + 2] < 90) continue
        ink.left = Math.min(ink.left, x)
        ink.right = Math.max(ink.right, x + 1)
        ink.top = Math.min(ink.top, y)
        ink.bottom = Math.max(ink.bottom, y + 1)
      }
    }
    assert.ok(Number.isFinite(ink.left), `${description}: visible glyph`)
    const inkOffset = {
      x: (crop.x + (ink.left + ink.right) / 2) / ratio - badge.x - badge.width / 2,
      y: (crop.y + (ink.top + ink.bottom) / 2) / ratio - badge.y - badge.height / 2,
    }
    const rasterOffset = {
      x: (crop.x + (ink.left + ink.right) / 2) / ratio - icon.glyphCenter.x,
      y: (crop.y + (ink.top + ink.bottom) / 2) / ratio - icon.glyphCenter.y,
    }
    assert.ok(Math.abs(rasterOffset.x) <= 1 && Math.abs(rasterOffset.y) <= 1,
      `${description}: raster must match the centered artwork within one CSS pixel ${JSON.stringify({ rasterOffset, ratio, badge, glyphCenter: icon.glyphCenter, crop, ink })}`)
    iconMeasurements.push({ page, ...icon, inkOffset, rasterOffset })
  }
}
async function capture(name) {
  await paint()
  await verifyContentHierarchy(name)
  await evaluate("Promise.all(document.getAnimations().map(animation => animation.finished.catch(() => {})))")
  await evaluate(`new Promise(resolve => {
    // Chromium's internal progress fill is absent from document.getAnimations().
    const durations = [...document.styleSheets].flatMap(sheet => [...sheet.cssRules])
      .filter(rule => rule.selectorText === 'progress::-webkit-progress-value')
      .map(rule => parseFloat(rule.style.transitionDuration) * 1000);
    const deadline = performance.now() + Math.max(0, ...durations);
    const frame = () => performance.now() >= deadline ? resolve() : requestAnimationFrame(frame);
    requestAnimationFrame(frame);
  })`)
  await paint()
  await verifyPageGeometry(name)
  await verifyIcons(name)
  await fs.writeFile(path.join(output, name + '.png'), (await window.webContents.capturePage()).toPNG())
  window.setContentSize(720, 560)
  await paint()
  assert.equal(await evaluate('document.documentElement.scrollWidth <= innerWidth'), true)
  assert.equal(await evaluate(`(() => {
    const page = document.querySelector('.page:not([hidden])').getBoundingClientRect();
    const stage = document.getElementById('pages').getBoundingClientRect();
    const footer = document.querySelector('footer').getBoundingClientRect();
    return page.left >= stage.left && page.right <= stage.right + 1 &&
      page.top >= stage.top - 1 && footer.bottom <= innerHeight;
  })()`), true, `Compact ${name} must retain accessible content and footer`)
  await verifyPageGeometry(name + '-compact')
  await verifyIcons(name + '-compact')
  await fs.writeFile(path.join(output, name + '-compact.png'), (await window.webContents.capturePage()).toPNG())
  window.setContentSize(1180, 760)
  await paint()
}
ipcMain.on('setup-command', (_event, serialized) => {
  void (async () => {
    const command = JSON.parse(serialized)
    commands.push(command.operation)
    let data = {}
    if (command.operation === 'ready' || command.operation === 'console') return
    if (command.operation === 'open') {
      opening = command.id
      await evaluate(`document.body.dataset.launchRequest = ${JSON.stringify(command.id)}`)
      return
    }
    if (command.operation === 'close') {
      await evaluate("document.body.dataset.closeRequested = 'true'")
      return
    }
    if (command.operation === 'locale') data = await evaluate('window.setupBootstrap.messages')
    if (command.operation === 'location') data = { comfyRoot: 'D:\\Test\\ComfyUI', models: 'D:\\Test\\models', outputs: 'D:\\Test\\outputs' }
    if (command.operation === 'location' && maintenance) data.existing = {
      selection: { installRoot: command.input.directory, comfyRoot: 'D:\\Saved\\ComfyUI' }, executableMissing: maintenance === true, launchEntryMissing: maintenance === 'missing-entry',
    }
    if (command.operation === 'options') data = [{
      value: 'choice', label: { release: 'Stable', comfyVersion: 'v0.42.1', variant: 'NVIDIA' }[command.input.field],
      description: command.input.field === 'variant' ? 'ComfyUI 0.42.1 · Python 3.13.12 · 2256 MB' : '',
      recommended: true, selectionToken: 'issued-by-test-boundary',
    }]
    if (['install', 'maintain'].includes(command.operation)) { install = command.id; return }
    if (command.operation === 'runtime') {
      inspection = command.id
      inspections.push(command.input)
      await evaluate("document.body.dataset.inspectionPending = 'true'")
      return
    }
    await deliver({ id: command.id, event: 'completed', data })
  })().catch(cause => { failure = cause })
})
void app.whenReady().then(async () => {
  window = new BrowserWindow({
    width: 1180, height: 760, useContentSize: true, show: false, backgroundColor: '#181818',
    webPreferences: { offscreen: true, contextIsolation: true, nodeIntegration: false, preload: path.join(__dirname, 'surface-preload.cjs') },
  })
  await window.loadFile(source)
  copy = await evaluate('window.setupBootstrap.messages')
  assert.equal(await evaluate('document.documentElement.lang'), await evaluate('window.setupBootstrap.locale'))
  assert.equal(await evaluate('document.title'), copy.title)
  assert.equal(await evaluate("document.querySelector('#brand svg desc').textContent"), copy.wordmarkDescription)
  // Match the Python offscreen capture's opaque backing beneath the native-material surface.
  await evaluate("document.documentElement.style.background = '#181818'")
  await until("!document.getElementById('language').hidden")
  assert.equal(await evaluate('document.querySelector(".wordmark svg") !== null'), true)
  assert.equal(await evaluate("document.querySelector('#language .hero-icon svg') !== null"), true)
  const languageGeometry = await evaluate(`(() => {
    const page = document.getElementById('language').getBoundingClientRect();
    const stage = document.getElementById('pages').getBoundingClientRect();
    const icon = document.querySelector('#language .hero-icon').getBoundingClientRect();
    const title = document.querySelector('#language h1').getBoundingClientRect();
    return { centerDifference: Math.abs((page.top + page.bottom - stage.top - stage.bottom) / 2),
      iconBesideTitle: icon.right < title.left && Math.abs(icon.top - title.top) < 6,
      buttonHeight: document.getElementById('primary').getBoundingClientRect().height };
  })()`)
  assert.ok(languageGeometry.centerDifference < 2)
  assert.equal(languageGeometry.iconBesideTitle, true)
  assert.equal(languageGeometry.buttonHeight, 32)
  await capture('language')
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('location').hidden")
  await evaluate("document.getElementById('installRoot').value = 'D:\\\\Apps\\\\SugarSubstitute'")
  await capture('location')
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('runtime').hidden && !document.getElementById('primary').disabled")
  assert.equal(await evaluate("document.querySelector('#runtime [role=radiogroup]').getAttribute('aria-labelledby')"), 'runtimeTitle')
  assert.equal(commands.filter(value => value === 'options').length, 0)
  await capture('runtime')
  await evaluate("document.querySelector('input[name=mode][value=attached]').click()")
  assert.equal(await evaluate("document.querySelector('[data-choice-label=attached]').textContent"), copy.selected)
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('configuration').hidden")
  assert.equal(await evaluate("document.getElementById('attached').hidden"), false)
  assert.equal(commands.filter(value => value === 'options').length, 0)
  await capture('configuration-existing')
  await evaluate('document.getElementById("primary").click()')
  await until("document.body.dataset.inspectionPending === 'true'")
  assert.equal(inspections.at(-1).mode, 'attached')
  assert.equal(inspections.at(-1).comfyRoot, 'D:\\Test\\ComfyUI')
  await deliver({ id: inspection, event: 'completed', data: {} })
  await until("!document.getElementById('folders').hidden")
  await evaluate("delete document.body.dataset.inspectionPending")
  await evaluate('document.getElementById("back").click()')
  await evaluate('document.getElementById("back").click()')
  await evaluate("document.querySelector('input[name=mode][value=managed]').click()")
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('configuration').hidden && !document.getElementById('primary').disabled")
  assert.deepEqual(commands.filter(value => value === 'options'), ['options', 'options', 'options'])
  await capture('configuration')
  await evaluate('document.getElementById("primary").click()')
  await until("document.body.dataset.inspectionPending === 'true'")
  assert.equal(inspections.at(-1).mode, 'managed')
  await elapsed(121000)
  await until("!document.getElementById('wait').hidden")
  assert.equal(await evaluate("document.getElementById('primary').disabled"), true)
  await deliver({ id: inspection, event: 'completed', data: {} })
  await until("!document.getElementById('folders').hidden")
  assert.equal(await evaluate("document.getElementById('wait').hidden"), true)
  await elapsed(0)
  await capture('folders')
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('progress').hidden")
  assert.ok(install)
  assert.equal(await evaluate("document.getElementById('status').textContent"), copy.preparingInstallation)
  await verifyContentHierarchy('progress-initial')
  await evaluate("document.getElementById('installRoot').value = 'D:\\\\Unsubmitted'")
  await deliver({ event: 'progress', data: { status: 'Installing Python dependencies', overall: 42 } })
  await capture('progress-collapsed')
  await evaluate('document.getElementById("details").open = true')
  for (let index = 0; index < 180; index++) {
    await deliver({ event: 'progress', data: { status: `Provisioning line ${index}: installing Python packages`, overall: index / 3 } })
  }
  await paint()
  assert.equal(await evaluate("document.getElementById('console').textContent"), '')
  assert.equal(await evaluate("document.getElementById('console').tagName"), 'DIV')
  assert.equal(await evaluate("document.querySelector('#details summary').textContent"), copy.hideDetails)
  await deliver({ event: 'progress', data: { status: 'Latest final output', overall: 62 } })
  await paint()
  await deliver({ event: 'progress', data: { status: 'Next phase', overall: 2 } })
  assert.equal(await evaluate("document.getElementById('total').value"), 62)
  await elapsed(121000)
  await until("!document.getElementById('wait').hidden")
  assert.equal(await evaluate("document.getElementById('total').value"), 62)
  await elapsed(0)
  await capture('progress')
  await evaluate('document.getElementById("details").open = false')
  await capture('progress-wait')
  await evaluate('document.getElementById("details").open = true')
  for (const size of [[720, 560], [1180, 760]]) {
    window.setContentSize(...size)
    await paint()
    assert.equal(await evaluate(`Array.from(document.querySelectorAll('body, main, #progress, #details, #console')).filter(element => {
      const overflow = getComputedStyle(element).overflowY;
      return ['auto', 'scroll'].includes(overflow) && element.scrollHeight > element.clientHeight + 1;
    }).length`), 0)
    assert.ok(await evaluate("document.getElementById('console').getBoundingClientRect().height > 8"))
  }
  await deliver({ id: install, event: 'completed', data: { ready: true } })
  await until("!document.getElementById('complete').hidden")
  assert.equal(await evaluate("document.getElementById('wait').hidden"), true)
  assert.equal(await evaluate("document.getElementById('installedPath').textContent"), 'D:\\Apps\\SugarSubstitute')
  assert.equal(await evaluate("document.querySelectorAll('#complete svg').length"), 1)
  await capture('complete')
  const freshCommands = commands.length
  maintenance = true
  await window.loadFile(source)
  copy = await evaluate('window.setupBootstrap.messages')
  assert.equal(await evaluate('document.documentElement.lang'), await evaluate('window.setupBootstrap.locale'))
  assert.equal(await evaluate('document.title'), copy.title)
  await until("!document.getElementById('language').hidden")
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('location').hidden")
  await evaluate("document.getElementById('installRoot').value = 'D:\\\\Existing\\\\Substitute'")
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('maintenance').hidden")
  assert.equal(await evaluate("document.getElementById('journey').max"), 5)
  assert.equal(await evaluate("document.getElementById('maintenanceComfy').textContent"), 'D:\\Saved\\ComfyUI')
  assert.equal(await evaluate("document.getElementById('maintenanceNotice').textContent"),
    copy.maintenanceMissing)
  await capture('maintenance')
  await evaluate('document.getElementById("back").click()')
  await until("!document.getElementById('location').hidden")
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('maintenance').hidden")
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('progress').hidden")
  await deliver({ id: install, event: 'failed', data: { message: 'Qualification retry' } })
  await until("!document.getElementById('primary').hidden")
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('maintenance').hidden")
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('progress').hidden")
  await evaluate("document.getElementById('installRoot').value = 'D:\\\\Unsubmitted'")
  await deliver({ id: install, event: 'completed', data: { ready: true } })
  await until("!document.getElementById('complete').hidden")
  assert.equal(await evaluate("document.getElementById('installedPath').textContent"), 'D:\\Existing\\Substitute')
  assert.deepEqual(commands.slice(freshCommands).filter(value =>
    ['options', 'runtime', 'validate', 'install', 'maintain'].includes(value)), ['maintain', 'maintain'])
  const beforeLaunch = commands.length
  await evaluate('document.getElementById("primary").click()')
  await until('document.body.dataset.launchRequest !== undefined')
  const firstOpen = opening
  assert.equal(await evaluate('document.getElementById("primary").disabled'), true)
  await evaluate('document.getElementById("primary").click()')
  assert.equal(commands.slice(beforeLaunch).filter(value => value === 'open').length, 1)
  await deliver({ id: firstOpen, event: 'failed', data: { message: copy.setupFailed } })
  await until('!document.getElementById("primary").disabled && !document.getElementById("error").hidden')
  assert.equal(await evaluate('document.getElementById("complete").hidden'), false)
  assert.equal(await evaluate('document.getElementById("error").textContent'), copy.setupFailed)
  assert.equal(commands.slice(beforeLaunch).includes('close'), false)
  await evaluate('document.getElementById("primary").click()')
  await until(`document.body.dataset.launchRequest !== ${JSON.stringify(firstOpen)}`)
  assert.equal(await evaluate('document.getElementById("error").hidden'), true)
  assert.deepEqual(commands.slice(beforeLaunch).filter(value =>
    ['install', 'maintain', 'open', 'close'].includes(value)), ['open', 'open'])
  await deliver({ id: opening, event: 'completed', data: {} })
  await until("document.body.dataset.closeRequested === 'true'")
  assert.deepEqual(commands.slice(beforeLaunch).filter(value =>
    ['install', 'maintain', 'open', 'close'].includes(value)), ['open', 'open', 'close'])
  maintenance = 'missing-entry'
  const beforeRecovery = commands.length
  await window.loadFile(source)
  copy = await evaluate('window.setupBootstrap.messages')
  assert.equal(await evaluate('document.documentElement.lang'), await evaluate('window.setupBootstrap.locale'))
  assert.equal(await evaluate('document.title'), copy.title)
  await until("!document.getElementById('language').hidden")
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('location').hidden")
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('maintenance').hidden")
  assert.equal(await evaluate("document.getElementById('maintenanceNotice').textContent"),
    copy.maintenanceLaunchMissing)
  await capture('maintenance-recovery')
  await evaluate('document.getElementById("primary").click()')
  await until("!document.getElementById('progress').hidden")
  assert.deepEqual(commands.slice(beforeRecovery).filter(value =>
    ['options', 'runtime', 'validate', 'install', 'maintain'].includes(value)), ['maintain'])
  if (failure) throw failure
  await fs.writeFile(path.join(output, 'surface-result.json'), JSON.stringify({
    passed: true, scale: Number(scale), commands, nativeConsolePlacement: true, duplicateHtmlScrollbars: false,
    testedSizes: [[720,560],[1180,760]], progressMonotonic: true, slowInspectionAndInstallationFeedback: true,
    compactCenteredLanguage: languageGeometry, separateChoiceAndConfiguration: true,
    inspectedModes: inspections.map(input => input.mode),
    locale: await evaluate('window.setupBootstrap.locale'), localizedTitle: copy.title, maintenanceBypassesProvisioning: true, missingEntryRecovery: true, maintenanceRetryPreservesRoute: true,
    launchRetryKeepsCompletionPage: true, duplicateOpenSuppressed: true, closeFollowsSuccessfulOpen: true,
    iconMeasurements,
    contentHierarchy: { uniqueDecorativeIcons: true, singlePageHeading: true, completionShowsSubmittedFolder: true },
  }, null, 2))
  window.destroy()
  app.quit()
}).catch(async cause => {
  await fs.writeFile(path.join(output, 'surface-result.json'), JSON.stringify({ passed: false, error: String(cause), stack: cause.stack }))
  if (window && !window.isDestroyed()) {
    await fs.writeFile(path.join(output, 'failure.png'), (await window.webContents.capturePage()).toPNG())
  }
  if (window) window.destroy()
  app.exit(1)
})
