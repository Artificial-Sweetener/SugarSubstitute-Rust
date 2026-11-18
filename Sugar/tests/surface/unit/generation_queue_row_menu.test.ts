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
/** Prove a finished queue row offers its job's seeds, and that they land on the matching Cubes. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  GenerationQueueRowMenu,
  USE_JOB_SEEDS,
  type QueueJobSeed,
} from '../../../frontend/comfyui/ui/surface/frame/GenerationQueueRowMenu.js';
import { NativeJobSeedAdoption } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeJobSeedAdoption.js';
import type { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import type { NativeRelationInventory } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationInventory.js';

afterEach(() => {
  document.body.replaceChildren();
});

/** Mount Comfy's job list with one row per title, and its job store behind the Vue root. */
function queue(tasks: readonly { jobId: string; previewOutput?: { filename: string } }[]): void {
  const root = document.createElement('div');
  root.id = 'vue-app';
  Reflect.set(root, '__vue_app__', {
    config: { globalProperties: { $pinia: { _s: new Map([['queue', { tasks }]]) } } },
  });
  const list = document.createElement('div');
  list.dataset.testid = 'job-assets-list';
  for (const title of ['apple.png', 'twice.png', 'Job queued']) {
    const slot = document.createElement('div');
    slot.className = 'h-12';
    const row = document.createElement('div');
    const text = document.createElement('div');
    text.className = 'flex-col';
    const heading = document.createElement('div');
    heading.textContent = title;
    text.append(heading);
    row.append(text);
    slot.append(row);
    list.append(slot);
  }
  document.body.append(root, list);
}

/** Right-click one row and return the menu entry the generation queue adds. */
function entryFor(title: string): HTMLElement | null {
  const heading = [...document.querySelectorAll('.flex-col > div')].find(
    (element) => element.textContent === title,
  );
  heading?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  return (
    [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
      (item) => item.textContent.trim() === USE_JOB_SEEDS,
    ) ?? null
  );
}

test('a finished row takes its job’s seeds with every digit, read from the reply text', async () => {
  queue([
    { jobId: 'job-apple', previewOutput: { filename: 'apple.png' } },
    { jobId: 'job-a', previewOutput: { filename: 'twice.png' } },
    { jobId: 'job-b', previewOutput: { filename: 'twice.png' } },
    { jobId: 'job-waiting' },
  ]);
  const historyText = '{"job-apple":{"prompt":[1,"job-apple",{"seed":12345678901234567890}]}}';
  const calls: { path: string; body: string | null }[] = [];
  const api = {
    fetchApi: jest.fn(async (path: string, init?: RequestInit) => {
      calls.push({ path, body: typeof init?.body === 'string' ? init.body : null });
      return path.startsWith('/history/')
        ? { ok: true, text: async () => historyText }
        : {
            ok: true,
            json: async () => ({
              seeds: [
                {
                  alias: 'Text to Image',
                  node_symbol: 'ksampler',
                  input_name: 'seed',
                  digits: '12345678901234567890',
                },
                { alias: 'unreadable' },
              ],
            }),
          };
    }),
  };
  const adopted: (readonly QueueJobSeed[])[] = [];
  const notices: string[] = [];
  const report = jest.fn();
  const menu = new GenerationQueueRowMenu({
    document,
    app: { api },
    adoptSeeds: (seeds) => {
      adopted.push(seeds);
      return seeds.length;
    },
    notify: (severity, title) => notices.push(`${severity}: ${title}`),
    report,
  });

  const entry = entryFor('apple.png');
  expect(entry?.getAttribute('aria-disabled')).not.toBe('true');
  entry?.click();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(calls.map((call) => call.path)).toEqual([
    '/history/job-apple',
    '/sugarcubes/v2/queue/job-seeds',
  ]);
  // The reply travels as text, so no digit of the seed is rounded on the way.
  expect(JSON.parse(calls[1]?.body ?? '{}')).toEqual({ history_text: historyText });
  expect(adopted).toEqual([
    [
      {
        alias: 'Text to Image',
        nodeSymbol: 'ksampler',
        inputName: 'seed',
        digits: '12345678901234567890',
      },
    ],
  ]);
  expect(notices).toEqual(['success: Seeds taken from the job']);
  expect(report).not.toHaveBeenCalled();
  menu.dispose();
});

test('a row that names no single job offers nothing to take', () => {
  queue([
    { jobId: 'job-a', previewOutput: { filename: 'twice.png' } },
    { jobId: 'job-b', previewOutput: { filename: 'twice.png' } },
    { jobId: 'job-waiting' },
  ]);
  const adoptSeeds = jest.fn(() => 0);
  const menu = new GenerationQueueRowMenu({
    document,
    app: {},
    adoptSeeds,
    notify: () => undefined,
    report: () => undefined,
  });
  for (const title of ['twice.png', 'Job queued']) {
    const entry = entryFor(title);
    expect(entry?.getAttribute('aria-disabled')).toBe('true');
    entry?.click();
  }
  expect(adoptSeeds).not.toHaveBeenCalled();
  menu.dispose();
  // Once put away, a right-click on a row is Comfy's again.
  const later = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
  document.querySelector('.flex-col > div')?.dispatchEvent(later);
  expect(later.defaultPrevented).toBe(false);
});

test('seeds land on the Cubes of the same name, once for a seed every Cube shares', () => {
  const field = (id: string, nodeId: string, semantic = 'seed') => ({ id, nodeId, semantic });
  const node = (instance: string, symbol: string) => JSON.stringify(['cube', instance, symbol]);
  const fields = [
    field('a', node('one', 'ksampler')),
    field('b', node('two', 'ksampler')),
    field('c', node('two', 'variation')),
    field('d', node('two', 'ksampler'), 'steps'),
  ];
  const snapshot = {
    fields,
    nodes: [
      { id: node('one', 'ksampler'), cubeLabel: 'Text to Image' },
      { id: node('two', 'ksampler'), cubeLabel: 'Upscale' },
      { id: node('two', 'variation'), cubeLabel: 'Upscale' },
    ],
    resolved: [
      { field: fields[0], override: 'seed' },
      { field: fields[1], override: 'seed' },
      { field: fields[2], override: null },
    ],
  };
  const reuseSeed = jest.fn();
  const relations = { reconcile: () => snapshot, reuseSeed } as unknown as WorkflowRelations;
  const owners = new Map(
    fields.map((item) => [
      item.id,
      { nodeId: item.nodeId, cubeId: item.id === 'a' ? 'one' : 'two', inputName: 'seed' },
    ]),
  );
  const graph: { extra?: { sugarcubes_composition?: { value_relations?: unknown[] } } } = {};
  const adoption = new NativeJobSeedAdoption(() => graph, relations, {
    owners,
  } as unknown as NativeRelationInventory);

  const taken = adoption.adopt([
    {
      alias: 'Text to Image',
      nodeSymbol: 'ksampler',
      inputName: 'seed',
      digits: '12345678901234567890',
    },
    { alias: 'Upscale', nodeSymbol: 'ksampler', inputName: 'seed', digits: '12345678901234567890' },
    { alias: 'Upscale', nodeSymbol: 'variation', inputName: 'noise_seed', digits: '4' },
    { alias: 'Elsewhere', nodeSymbol: 'ksampler', inputName: 'seed', digits: '9' },
    { alias: 'Upscale', nodeSymbol: 'variation', inputName: 'seed', digits: 'not digits' },
  ]);

  expect(taken).toBe(3);
  expect(reuseSeed.mock.calls).toEqual([
    ['a', Number('12345678901234567890')],
    ['c', 4],
  ]);
  const recorded = graph.extra?.sugarcubes_composition?.value_relations ?? [];
  expect(recorded).toHaveLength(3);
  expect(recorded[0]).toMatchObject({ kind: 'exact_integer', digits: '12345678901234567890' });
});
