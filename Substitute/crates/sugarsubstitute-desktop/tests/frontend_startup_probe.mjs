// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Run with Node's --experimental-vm-modules flag; the optional argument selects the Rust source.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { Script, SourceTextModule, SyntheticModule, createContext } from 'node:vm';

const source = readFileSync(
  process.argv[2] ?? new URL('../src/main_actor/evidence/probe.rs', import.meta.url),
  'utf8',
);
const samples = [...source.matchAll(/const SAMPLE: &str = r#"([\s\S]*?)"#;/g)];
assert.equal(samples.length, 1, 'Read the actual single Rust readiness sample');
const sample = samples[0][1];

function element(selectors = [], id = '') {
  return {
    selectors,
    id,
    isConnected: true,
    getBoundingClientRect: () => ({ width: 640, height: 480 }),
  };
}

// Fake only the external document and imported owner exports; the real SAMPLE decides readiness.
async function inspect({ splash = false, extra = [], graphReady = true, setupDone = true,
  vueReady = true, position = true, api = true, extensions = true, named = true,
  registered = true, graphSame = true, connected = true, width = 640,
  complete = true, configuring = false, library = true, apiAfterImport = true,
  earlyExtensionGetter = false, forbidImport = false, appPresent = true,
  apiBucketPresent = true, forbidDerived = false, frameApi = true, frame = {},
  setupOutcome = null } = {}) {
  const extension = { name: named ? 'SugarCubes.UI' : 'Other' };
  const graph = {};
  const elements = [...extra];
  if (splash) elements.push(element([], 'splash-loader'));
  const app = {
    isGraphReady: graphReady,
    vueAppReady: vueReady,
    positionConversion: position ? {} : null,
    extensions: extensions ? [extension] : null,
    graph,
    canvas: { graph: graphSame ? graph : {}, canvas: {
      ...element(), isConnected: connected,
      getBoundingClientRect: () => ({ width, height: 480 }),
    } },
    configuringGraph: configuring,
  };
  const context = createContext({
    window: { comfyAPI: apiBucketPresent ? { app: { app: appPresent ? app : undefined } } : undefined,
      SugarCubes: api ? {
        openLibrary: library ? () => {} : null,
        ...(frameApi ? {
          configureDesktopFrame: () => assert.fail('Readiness inspection must request host configuration'),
          readDesktopFrame: () => ({ mounted: true, schemaVersion: 1, captionWidth: 138,
            titlebarHeight: 34, darkTheme: true, ...frame }),
        } : {}),
      } : null },
    document: {
      readyState: complete ? 'complete' : 'loading',
      documentElement: {
        getAttribute: (name) => (name === 'data-sugarcubes-setup' ? setupOutcome : null),
      },
      getElementById: (id) => elements.find((item) => item.id === id) ?? null,
      querySelectorAll: (selector) => elements.filter((item) =>
        selector.split(',').some((part) => item.selectors.includes(part.trim()))),
    },
    getComputedStyle: () => ({ visibility: 'visible' }),
    LAUNCH: 11,
    NAV: 17,
    FRONT: '1.47.11',
    CUBES: '0.15.3',
    NATIVE_DARK: true,
  });
  if (earlyExtensionGetter) Object.defineProperty(app, 'extensions', {
    get() {
      if (!vueReady) {
        throw new TypeError('Pinia-backed extension store accessed before Vue mount');
      }
      return extensions ? [extension] : null;
    },
  });
  if (forbidDerived) {
    for (const key of ['positionConversion', 'graph', 'canvas', 'configuringGraph']) {
      Object.defineProperty(app, key, {
        get() { assert.fail(`Pre-graph sample must not read derived app.${key}`); },
      });
    }
    Object.defineProperty(context.window, 'SugarCubes', {
      get() { assert.fail('Pre-graph sample must not inspect the SugarCubes public API'); },
    });
    context.document.getElementById = () => assert.fail('Pre-graph sample must not inspect loader DOM');
  }
  const owner = new SyntheticModule(['sugarCubesExtension', 'sugarCubesUI'], function () {
    this.setExport('sugarCubesExtension', registered ? extension : { name: 'SugarCubes.UI' });
    this.setExport('sugarCubesUI', { _setupDone: setupDone });
  }, { context });
  await owner.link(() => assert.fail('The owner fake has no imports'));
  await owner.evaluate();
  const script = new Script(sample, {
    importModuleDynamically: (specifier) => {
      assert.equal(forbidImport, false, 'Incomplete earlier gates must not import the owner');
      assert.equal(specifier, '/extensions/SugarCubes/comfyui/ui.js');
      if (!apiAfterImport) context.window.SugarCubes = null;
      return owner;
    },
  });
  return JSON.parse(await script.runInContext(context));
}

test('completed startup transfers the actual receipt with current identities and versions', async () => {
  const receipt = await inspect();
  assert.equal(receipt.schema_version, 1);
  assert.equal(receipt.launch_id, 11);
  assert.equal(receipt.navigation_generation, 17);
  assert.deepEqual(receipt.outcome, {
    status: 'ready',
    evidence: {
      frontend_version: '1.47.11',
      sugarcubes_version: '0.15.3',
      app_graph_ready: true,
      canvas_connected: true,
      loading_finished: true,
      sugarcubes_registered: true,
      sugarcubes_ready: true,
    },
  });
});

test('the dedicated ComfyUI startup splash blocks readiness until removed', async () => {
  assert.deepEqual(await inspect({ splash: true }), { incomplete: 2048 });
});

test('an unmounted desktop frame requests configuration before granting readiness', async () => {
  assert.deepEqual(await inspect({ frame: { mounted: false } }), { configure_desktop_frame: 1 });
});

test('missing frame protocol methods reject readiness', async () => {
  await assert.rejects(inspect({ frameApi: false }), /desktop frame protocol 1 is unavailable/);
});

test('frame schema, native dimensions and selected theme must match before readiness', async () => {
  for (const frame of [{ schemaVersion: 2 }, { captionWidth: 137 },
    { titlebarHeight: 35 }, { darkTheme: false }]) {
    await assert.rejects(inspect({ frame }), /presentation differs from the native host/);
  }
});

test('an unrelated visible PrimeVue progress spinner does not block startup readiness', async () => {
  const receipt = await inspect({ extra: [element(['.p-progressspinner'])] });
  assert.equal(receipt?.outcome.status, 'ready');
});

test('an unrelated visible LoadingOverlay does not block startup readiness', async () => {
  const receipt = await inspect({ extra: [element(['[data-testid="loading-overlay"]'])] });
  assert.equal(receipt?.outcome.status, 'ready');
});

test('removing the startup splash does not replace actual graph readiness', async () => {
  assert.deepEqual(await inspect({ graphReady: false }), { incomplete: 1 });
});

test('removing the startup splash does not replace completed SugarCubes setup', async () => {
  assert.deepEqual(await inspect({ setupDone: false }), { incomplete: 4096 });
});

test('actual sample identifies each remaining blocked condition without receipt rights', async () => {
  for (const [options, incomplete] of [
    [{ vueReady: false }, 2], [{ position: false }, 4],
    [{ extensions: false }, 16],
    [{ registered: false }, 64], [{ graphSame: false }, 128],
    [{ connected: false }, 256], [{ width: 0 }, 256],
    [{ complete: false }, 512], [{ configuring: true }, 1024],
    [{ library: false }, 8192], [{ apiAfterImport: false }, 8192 | 16384],
  ]) assert.deepEqual(await inspect(options), { incomplete });
});

test('actual sample preserves simultaneous failed checks after the same owner import', async () => {
  assert.deepEqual(await inspect({ splash: true, configuring: true, setupDone: false }),
    { incomplete: 2048 | 1024 | 4096 });
});


test('pre-Vue gate keeps the Pinia getter unread and earlier initialization gates keep owner unimported', async () => {
  for (const [options, incomplete] of [
    [{ graphReady: false }, 1], [{ vueReady: false }, 2],
    [{ position: false }, 4],
  ]) assert.deepEqual(await inspect({ ...options, earlyExtensionGetter: true, forbidImport: true }),
    { incomplete });
});

function failure(code, detail) {
  return { schema_version: 1, launch_id: 11, navigation_generation: 17,
    outcome: { status: 'failed', failure: { code, detail } } };
}

const unavailable = failure('sugarcubes_unavailable', 'SugarCubes did not load in this page.');

test('after every extension settled, an absent SugarCubes API or extension is a terminal failure', async () => {
  for (const options of [{ api: false }, { named: false }, { api: false, named: false }]) {
    assert.deepEqual(await inspect({ ...options, forbidImport: true }), unavailable);
  }
});

test('before extensions settle, an absent SugarCubes stays an incomplete sample', async () => {
  assert.deepEqual(await inspect({ api: false, named: false, position: false, forbidImport: true }),
    { incomplete: 4 });
});

test('a setup failure SugarCubes recorded on the document is a terminal failure', async () => {
  assert.deepEqual(await inspect({ setupDone: false, setupOutcome: 'failed', forbidImport: true }),
    failure('sugarcubes_failed', 'SugarCubes reported that its setup failed.'));
  assert.deepEqual(await inspect({ setupDone: false, setupOutcome: 'loaded' }), { incomplete: 4096 });
});

// Load fixture extension modules the way ComfyUI 1.47's extension service does: import each
// module, log and skip one that throws, run every registered setup, and only then assign
// positionConversion. The real SAMPLE then inspects the page those modules produced.
async function inspectLoadedPage(sources) {
  const graph = {};
  const extensions = [];
  const app = {
    isGraphReady: false,
    vueAppReady: true,
    positionConversion: null,
    get extensions() { return extensions; },
    registerExtension(extension) { extensions.push(extension); },
    graph,
    canvas: { graph, canvas: { ...element(), getBoundingClientRect: () => ({ width: 640, height: 480 }) } },
    configuringGraph: false,
  };
  const attributes = new Map();
  const window = { comfyAPI: { app: { app } } };
  const context = createContext({
    window,
    document: {
      readyState: 'complete',
      documentElement: {
        getAttribute: (name) => attributes.get(name) ?? null,
        setAttribute: (name, value) => attributes.set(name, String(value)),
      },
      getElementById: () => null,
      querySelectorAll: () => [],
    },
    getComputedStyle: () => ({ visibility: 'visible' }),
    LAUNCH: 11, NAV: 17, FRONT: '1.47.11', CUBES: '0.15.3', NATIVE_DARK: true,
  });
  const scripts = new SyntheticModule(['app'], function () { this.setExport('app', app); },
    { context });
  const errors = [];
  for (const source of sources) {
    const module = new SourceTextModule(source, { context });
    await module.link((specifier) => {
      assert.equal(specifier, '/scripts/app.js');
      return scripts;
    });
    try {
      await module.evaluate();
    } catch (error) {
      errors.push(error);
    }
  }
  app.isGraphReady = true;
  await Promise.all(extensions.map(async (extension) => {
    try { await extension.setup?.(); } catch (error) { errors.push(error); }
  }));
  app.positionConversion = {};
  const script = new Script(sample, {
    importModuleDynamically: () => assert.fail('A SugarCubes that failed must not be imported'),
  });
  return { receipt: JSON.parse(await script.runInContext(context)), errors };
}

test('a SugarCubes UI module that throws while loading reports SugarCubes unavailable', async () => {
  const { receipt, errors } = await inspectLoadedPage([
    `import { app } from '/scripts/app.js';
     throw new Error('Fixture: this SugarCubes UI cannot load');
     app.registerExtension({ name: 'SugarCubes.UI' });`,
    `import { app } from '/scripts/app.js';
     app.registerExtension({ name: 'SugarCubes.PageLoadReport', setup() {} });`,
  ]);
  assert.equal(errors.length, 1);
  assert.match(errors[0].message, /cannot load/);
  assert.deepEqual(receipt, unavailable);
});

test('a SugarCubes UI whose setup throws and records it reports SugarCubes failed', async () => {
  const { receipt, errors } = await inspectLoadedPage([
    `import { app } from '/scripts/app.js';
     app.registerExtension({ name: 'SugarCubes.UI', async setup() {
       document.documentElement.setAttribute('data-sugarcubes-setup', 'failed');
       throw new Error('Fixture: this SugarCubes setup cannot finish');
     } });
     window.SugarCubes = { openLibrary() {} };`,
  ]);
  assert.equal(errors.length, 1);
  assert.deepEqual(receipt, failure('sugarcubes_failed', 'SugarCubes reported that its setup failed.'));
});

// Pre-Vue masks leave extension-array bit16 unknown; no Pinia-backed getter is invoked.
test('absent app or API bucket reports truthful graph and Vue absence without derived reads', async () => {
  for (const options of [{ appPresent: false }, { apiBucketPresent: false }]) {
    assert.deepEqual(await inspect({ ...options, forbidDerived: true, forbidImport: true }),
      { incomplete: 1 | 2 });
  }
});

test('existing app before Vue mount reports both missing stages without touching Pinia or derived getters', async () => {
  assert.deepEqual(await inspect({ graphReady: false, vueReady: false,
    earlyExtensionGetter: true, forbidDerived: true, forbidImport: true }),
    { incomplete: 1 | 2 });
});

test('Vue mounted while graph is absent samples the graph-independent extension array safely', async () => {
  assert.deepEqual(await inspect({ graphReady: false, vueReady: true,
    earlyExtensionGetter: true, forbidDerived: true, forbidImport: true }), { incomplete: 1 });
});

test('Vue mounted graph wait co-reports an actually absent extension array without importing or reading graph state', async () => {
  assert.deepEqual(await inspect({ graphReady: false, vueReady: true, extensions: false,
    earlyExtensionGetter: true, forbidDerived: true, forbidImport: true }),
    { incomplete: 1 | 16 });
});
