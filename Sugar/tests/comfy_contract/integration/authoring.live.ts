//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Exercise authoring, projection and serialization through the running pack's public entry points. */
import { test, expect } from '../support/LiveComfy.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import { command, importIntoBlank, setting } from '../support/NativeWorkflow.js';
import type { snapshotInstance } from '../../../frontend/comfyui/ui/graph/DirtySnapshotter.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';

test('dirty snapshots preserve native link endpoints and distinguish edits from moving a whole Cube', async ({
  loaded: page,
}) => {
  const observed = await page.evaluate(async () => {
    const { app } = await import('/scripts/app.js');
    const module = '/extensions/SugarCubes/comfyui/ui/graph/DirtySnapshotter.js';
    const dirty: { snapshotInstance: typeof snapshotInstance } = await import(module);
    // Comfy loads and owns these graph methods outside its published extension types.
    const host = app as unknown as {
      loadGraphData(graph: unknown): Promise<void>;
      graph: ComfyGraph & {
        getNodeById(id: number): {
          pos: [number, number];
          widgets: { name: string; value: unknown }[];
        };
      };
    };
    await host.loadGraphData({
      version: 0.4,
      last_node_id: 2,
      last_link_id: 1,
      nodes: [
        {
          id: 1,
          type: 'EmptyImage',
          pos: [100, 100],
          size: [270, 140],
          flags: {},
          order: 0,
          mode: 0,
          inputs: [],
          outputs: [{ name: 'IMAGE', type: 'IMAGE', links: [1] }],
          properties: {},
          widgets_values: [32, 16, 1, 16711680],
        },
        {
          id: 2,
          type: 'ImageInvert',
          pos: [480, 160],
          size: [140, 80],
          flags: {},
          order: 1,
          mode: 0,
          inputs: [{ name: 'image', type: 'IMAGE', link: 1 }],
          outputs: [{ name: 'IMAGE', type: 'IMAGE', links: [] }],
          properties: {},
          widgets_values: [],
        },
      ],
      links: [[1, 1, 0, 2, 0, 'IMAGE']],
      groups: [],
      config: {},
      extra: {},
    });
    const capture = (x: number, y: number) =>
      dirty.snapshotInstance(host.graph, [1, 2], [], [x, y], null);
    const before = capture(100, 100);
    for (const id of [1, 2]) {
      const node = host.graph.getNodeById(id);
      node.pos = [node.pos[0] + 80, node.pos[1] + 30];
    }
    const moved = capture(180, 130);
    const width = host.graph.getNodeById(1).widgets.find((w) => w.name === 'width');
    if (!width) throw new Error('The EmptyImage width widget is missing');
    width.value = 48;
    const edited = capture(180, 130);
    width.value = 32;
    return { before, moved, edited, restored: capture(180, 130) };
  });
  expect(observed.before.links).toEqual([
    // DirtySnapshotter deliberately normalizes LiteGraph's renderer-specific link type.
    { origin: '1', origin_slot: 0, target: '2', target_slot: 0, type: '' },
  ]);
  expect(observed.before.nodes.map((n) => n.pos)).toEqual([
    [0, 0],
    [380, 60],
  ]);
  expect(observed.moved).toEqual(observed.before);
  expect(observed.edited.nodes[0]!.widgets).toContainEqual({ name: 'width', value: 48 });
  expect(observed.edited).not.toEqual(observed.before);
  expect(observed.restored).toEqual(observed.before);
});

const script = [
  'use "Artificial-Sweetener/Base-Cubes/Compatibility.cube" as Left',
  'use "Artificial-Sweetener/Base-Cubes/Compatibility.cube" as Right',
  'set Left.color.width = 48',
  'set Right.color.width = 64',
].join('\n');

test('prompt relations expose only earlier compatible Cubes and restore independent editing', async ({
  loaded: page,
}) => {
  const cube = 'use "Artificial-Sweetener/Base-Cubes/PromptStudio.cube" as ';
  await importIntoBlank(page, cube + 'First\n' + cube + 'Second');
  await command(page, 'SugarCubes.ToggleMasonry');
  const masonry = page.locator('.sugarcubes-workflow-masonry');
  const editors = masonry.locator('.cm-content:visible');
  await expect(editors).toHaveCount(2);
  const selector = masonry.locator('.sc-node-link:visible');
  await expect(selector).toHaveCount(1);
  await selector.getByText('Independent', { exact: true }).click();
  await page.getByText('🔗 First', { exact: true }).last().click();
  await expect(editors).toHaveCount(1);
  await expect(editors).toHaveText('sunlit garden');
  await selector.getByText('🔗 First', { exact: true }).click();
  await page.getByText('Independent', { exact: true }).last().click();
  await expect(editors).toHaveCount(2);
});

test('editing a projected Cube field in masonry persists the change when the card remounts', async ({
  loaded: page,
}) => {
  await setting(page, 'Comfy.VueNodes.Enabled', true);
  await importIntoBlank(page, 'use "Artificial-Sweetener/Base-Cubes/Compatibility.cube" as Swatch');
  await command(page, 'SugarCubes.ToggleMasonry');
  const masonry = page.locator('.sugarcubes-workflow-masonry');
  const row = masonry.getByTestId('node-widget').filter({ has: page.getByText(/^height$/i) });
  const field = row.locator('input');
  await expect(field).toHaveValue('16');
  await field.fill('24');
  await field.press('Enter');
  await expect(field).toHaveValue('24');
  await command(page, 'SugarCubes.ToggleMasonry');
  await expect(masonry).toBeHidden();
  await command(page, 'SugarCubes.ToggleMasonry');
  await expect(field).toHaveValue('24');
});

test('two authored Cubes retain distinct aliases and widths through live graph serialization and script replay', async ({
  loaded: page,
  request,
}) => {
  const installed = await page.evaluate(async (source) => {
    const { app } = await import('/scripts/app.js');
    const graph = app as typeof app & {
      loadGraphData(value: unknown): Promise<void>;
      graphToPrompt(): Promise<{ workflow: unknown }>;
    };
    await graph.loadGraphData({
      version: 0.4,
      last_node_id: 0,
      last_link_id: 0,
      nodes: [],
      links: [],
      groups: [],
      config: {},
      extra: {},
    });
    const receipt: unknown = await window.SugarCubes.importSugarScript(source);
    return { receipt, workflow: (await graph.graphToPrompt()).workflow };
  }, script);
  expect(installed.receipt).toMatchObject({
    instanceIds: [expect.any(String), expect.any(String)],
  });
  const rendered = await request.post('/sugarcubes/v2/sugarscript/render', {
    data: { workflow: installed.workflow },
  });
  await expect(rendered).toBeOK();
  const document: unknown = await rendered.json();
  if (!isRecord(document) || typeof document.source !== 'string')
    throw new Error('Missing exported SugarScript');
  expect(document.source).toContain(' as Left');
  expect(document.source).toContain(' as Right');
  expect(document.source).toContain('set Left.color.Width = 48');
  expect(document.source).toContain('set Right.color.Width = 64');
  const replay = await request.post('/sugarcubes/v2/sugarscript/compile', {
    data: { source: document.source },
  });
  await expect(replay).toBeOK();
  expect(await replay.json()).toMatchObject({
    diagnostics: [],
    plan: { instances: [{ alias: 'Left' }, { alias: 'Right' }] },
  });
});

test('invalid SugarScript returns a located diagnostic and leaves the live graph intact', async ({
  loaded: page,
  request,
}) => {
  const source = 'set Missing.sampler.steps = 24\n';
  const response = await request.post('/sugarcubes/v2/sugarscript/compile', { data: { source } });
  expect(response.status()).toBe(422);
  expect(await response.json()).toMatchObject({
    plan: null,
    diagnostics: [{ code: 'sugarscript.compile.invalid_semantics', span: { start: { line: 1 } } }],
  });
  const unchanged = await page.evaluate(async (source) => {
    const { app } = await import('/scripts/app.js');
    const host = app as typeof app & { graphToPrompt(): Promise<{ output: unknown }> };
    const before = (await host.graphToPrompt()).output;
    try {
      await window.SugarCubes.importSugarScript(source);
    } catch {
      return { before, after: (await host.graphToPrompt()).output };
    }
    throw new Error('Malformed script was accepted');
  }, source);
  expect(unchanged.after).toEqual(unchanged.before);
});
