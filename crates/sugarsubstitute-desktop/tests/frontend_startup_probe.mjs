// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Run with Node's --experimental-vm-modules flag; the optional argument selects the Rust source.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { Script, SyntheticModule, createContext } from 'node:vm';

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
  complete = true, configuring = false, library = true, apiAfterImport = true } = {}) {
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
    window: { comfyAPI: { app: { app } }, SugarCubes: api ? { openLibrary: library ? () => {} : null } : null },
    document: {
      readyState: complete ? 'complete' : 'loading',
      getElementById: (id) => elements.find((item) => item.id === id) ?? null,
      querySelectorAll: (selector) => elements.filter((item) =>
        selector.split(',').some((part) => item.selectors.includes(part.trim()))),
    },
    getComputedStyle: () => ({ visibility: 'visible' }),
    LAUNCH: 11,
    NAV: 17,
    FRONT: '1.47.11',
    CUBES: '0.15.3',
  });
  const owner = new SyntheticModule(['sugarCubesExtension', 'sugarCubesUI'], function () {
    this.setExport('sugarCubesExtension', registered ? extension : { name: 'SugarCubes.UI' });
    this.setExport('sugarCubesUI', { _setupDone: setupDone });
  }, { context });
  await owner.link(() => assert.fail('The owner fake has no imports'));
  await owner.evaluate();
  const script = new Script(sample, {
    importModuleDynamically: (specifier) => {
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
    [{ vueReady: false }, 2], [{ position: false }, 4], [{ api: false }, 8],
    [{ extensions: false }, 16 | 32], [{ named: false }, 32],
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
