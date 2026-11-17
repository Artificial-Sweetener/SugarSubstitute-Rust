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
/** Prove a rebuilt workflow keeps the digits of whole numbers this page cannot hold. */
import { jest } from '@jest/globals';
import {
  readExactDigits,
  readExactIntegerFields,
  recordExactIntegers,
} from '../../../frontend/comfyui/ui/workflow/ExactIntegerRelations.js';
import { fixImportedSeeds } from '../../../frontend/comfyui/ui/workflow/ImportedSeedModes.js';
import { LegacyWorkflowAuthoringApi } from '../../../frontend/comfyui/ui/workflow/LegacyWorkflowAuthoringApi.js';
import { LegacyWorkflowImportService } from '../../../frontend/comfyui/ui/workflow/LegacyWorkflowImportService.js';
import type { CubeLibraryApi } from '../../../frontend/comfyui/ui/core/CubeLibraryApi.js';
import type { NativeWorkflowMaterializer } from '../../../frontend/comfyui/ui/workflow/NativeWorkflowMaterializer.js';

const SEED = '17066688399168319613';
const FIELD = { instance_id: 'cube-1', node_symbol: 'sampler', input_name: 'seed', digits: SEED };

test('only named fields with whole-number digits are accepted from the backend', () => {
  expect(
    readExactIntegerFields([
      FIELD,
      { ...FIELD, digits: '12e3' },
      { ...FIELD, digits: '007' },
      { ...FIELD, instance_id: 5 },
      'seed',
    ]),
  ).toEqual([{ instanceId: 'cube-1', nodeSymbol: 'sampler', inputName: 'seed', digits: SEED }]);
  expect(readExactIntegerFields(undefined)).toEqual([]);
});

test('digits are recorded as relations beside whatever the workflow already holds', () => {
  const root: { extra?: Record<string, unknown> } = {
    extra: {
      ds: { scale: 1 },
      sugarcubes_composition: {
        schema_version: 1,
        field_annotations: [{ annotation_id: 'kept' }],
        value_relations: [{ relation_id: 'link:kept', kind: 'field_link' }],
      },
    },
  };
  recordExactIntegers(root, readExactIntegerFields([FIELD]));
  expect(root.extra).toEqual({
    ds: { scale: 1 },
    sugarcubes_composition: {
      schema_version: 1,
      field_annotations: [{ annotation_id: 'kept' }],
      value_relations: [
        { relation_id: 'link:kept', kind: 'field_link' },
        {
          relation_id: 'exact:cube-1:sampler:seed:' + SEED,
          kind: 'exact_integer',
          digits: SEED,
          targets: [{ instance_id: 'cube-1', node_symbol: 'sampler', input_name: 'seed' }],
        },
      ],
    },
  });
  const bare: { extra?: unknown } = {};
  recordExactIntegers(bare, []);
  expect(bare.extra).toBeUndefined();
});

test('the saved text is what is sent, and digits are recorded only after the Cubes exist', async () => {
  const fetchJson = jest.fn(async (_path: string, _options: RequestInit) => ({
    response: { status: 200 },
    data: {
      valid: true,
      diagnostics: [],
      plan: { semantic_hash: 'a'.repeat(64), instances: [], connections: [] },
      exact_integers: [FIELD],
    },
  }));
  const order: string[] = [];
  const service = new LegacyWorkflowImportService(
    new LegacyWorkflowAuthoringApi({ fetchJson } as unknown as CubeLibraryApi),
    {
      materializeWorkflow: () => {
        order.push('materialized');
        return { semanticHash: 'h', instanceIds: [], warnings: [] };
      },
    } as unknown as NativeWorkflowMaterializer,
    (fields) => order.push('recorded ' + fields.map((field) => field.digits).join()),
    (instanceIds) => order.push('fixed ' + String(instanceIds.length)),
  );
  const text = `{"extra":{"seed":${SEED}}}`;
  await service.import(JSON.parse(text) as Record<string, unknown>, [0, 0], text);
  expect(JSON.parse(String(fetchJson.mock.calls[0]?.[1].body))).toEqual({ workflow_text: text });
  expect(order).toEqual(['materialized', 'fixed 0', 'recorded ' + SEED]);

  // Without the text, the parsed object is sent as before.
  await service.import({ version: 0.4 }, [0, 0]);
  expect(JSON.parse(String(fetchJson.mock.calls[1]?.[1].body))).toEqual({
    workflow: { version: 0.4 },
  });
  // A workflow that carried native nodes keeps the seed modes it was saved with.
  expect(order.filter((step) => step.startsWith('fixed'))).toHaveLength(1);
});

test('the seeds of rebuilt Cubes are fixed, and only theirs', () => {
  const control = (value: string) => ({
    name: 'control_after_generate',
    type: 'combo',
    value,
    options: { values: ['fixed', 'increment', 'decrement', 'randomize'] },
  });
  const cube = (instance: string, mode: string) => ({
    properties: { sugarcubes_cube: { instance_id: instance } },
    subgraph: {
      nodes: [
        { widgets: [{ name: 'seed', type: 'number', value: 7 }, control(mode)] },
        { subgraph: { nodes: [{ widgets: [control(mode)] }] } },
      ],
    },
  });
  const rebuilt = cube('rebuilt', 'randomize');
  const other = cube('other', 'randomize');
  const loose = { widgets: [control('randomize')] };
  expect(fixImportedSeeds({ nodes: [rebuilt, other, loose] }, ['rebuilt'])).toBe(2);
  const modes = (item: ReturnType<typeof cube>): unknown[] => [
    item.subgraph.nodes[0]?.widgets?.[1]?.value,
    item.subgraph.nodes[1]?.subgraph?.nodes[0]?.widgets[0]?.value,
  ];
  expect(modes(rebuilt)).toEqual(['fixed', 'fixed']);
  expect(modes(other)).toEqual(['randomize', 'randomize']);
  expect(loose.widgets[0]?.value).toBe('randomize');
});

test('an input keeps the digits of each value it holds, and one rounding has one record', () => {
  const first = { instanceId: 'a', nodeSymbol: 'ksampler', inputName: 'seed' };
  const second = { instanceId: 'b', nodeSymbol: 'ksampler', inputName: 'seed' };
  const typed = '12345678901234567890';
  const graph: { extra?: unknown } = {};
  expect(readExactDigits(graph, first, Number(SEED))).toBeNull();
  recordExactIntegers(graph, [
    { ...first, digits: SEED },
    { ...second, digits: '9007199254740993' },
  ]);
  recordExactIntegers(graph, [{ ...first, digits: typed }]);
  // The image's seed and the typed one are two values the input alternates between.
  expect(readExactDigits(graph, first, Number(SEED))).toBe(SEED);
  expect(readExactDigits(graph, first, Number(typed))).toBe(typed);
  expect(readExactDigits(graph, first, 42)).toBeNull();
  expect(readExactDigits(graph, second, Number(SEED))).toBeNull();
  expect(readExactDigits(graph, second, Number('9007199254740993'))).toBe('9007199254740993');
  // Other digits with the same rounding replace the record: the box can show only one of them.
  const neighbour = '17066688399168319614';
  expect(Number(neighbour)).toBe(Number(SEED));
  recordExactIntegers(graph, [{ ...first, digits: neighbour }]);
  expect(readExactDigits(graph, first, Number(SEED))).toBe(neighbour);
  expect(readExactDigits(graph, first, Number(typed))).toBe(typed);
});

test('an input keeps a bounded number of digit records', () => {
  const input = { instanceId: 'a', nodeSymbol: 'ksampler', inputName: 'seed' };
  const graph: { extra?: { sugarcubes_composition?: { value_relations?: unknown[] } } } = {};
  const seeds = Array.from({ length: 12 }, (_, index) =>
    String(10n ** 18n + BigInt(index) * 4096n),
  );
  for (const digits of seeds) recordExactIntegers(graph, [{ ...input, digits }]);
  expect(graph.extra?.sugarcubes_composition?.value_relations).toHaveLength(8);
  expect(readExactDigits(graph, input, Number(seeds[0]))).toBeNull();
  expect(readExactDigits(graph, input, Number(seeds[11]))).toBe(seeds[11]);
});
