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
async function inspect({ splash = false, extra = [], graphReady = true, setupDone = true } = {}) {
  const extension = { name: 'SugarCubes.UI' };
  const graph = {};
  const elements = [...extra];
  if (splash) elements.push(element([], 'splash-loader'));
  const app = {
    isGraphReady: graphReady,
    vueAppReady: true,
    positionConversion: {},
    extensions: [extension],
    graph,
    canvas: { graph, canvas: element() },
    configuringGraph: false,
  };
  const context = createContext({
    window: { comfyAPI: { app: { app } }, SugarCubes: { openLibrary() {} } },
    document: {
      readyState: 'complete',
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
    this.setExport('sugarCubesExtension', extension);
    this.setExport('sugarCubesUI', { _setupDone: setupDone });
  }, { context });
  await owner.link(() => assert.fail('The owner fake has no imports'));
  await owner.evaluate();
  const script = new Script(sample, {
    importModuleDynamically: (specifier) => {
      assert.equal(specifier, '/extensions/SugarCubes/comfyui/ui.js');
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
  assert.equal(await inspect({ splash: true }), null);
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
  assert.equal(await inspect({ graphReady: false }), null);
});

test('removing the startup splash does not replace completed SugarCubes setup', async () => {
  assert.equal(await inspect({ setupDone: false }), null);
});
