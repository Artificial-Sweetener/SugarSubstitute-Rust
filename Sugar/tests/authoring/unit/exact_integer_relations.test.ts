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
    (_instanceIds, links) => order.push('linked ' + String(links.length)),
  );
  const text = `{"extra":{"seed":${SEED}}}`;
  await service.import(JSON.parse(text) as Record<string, unknown>, [0, 0], text);
  expect(JSON.parse(String(fetchJson.mock.calls[0]?.[1].body))).toEqual({ workflow_text: text });
  // Its seeds keep ComfyUI's own control mode; only the recorded digits are kept exactly.
  expect(order).toEqual(['materialized', 'recorded ' + SEED]);

  // Without the text, the parsed object is sent as before.
  await service.import({ version: 0.4 }, [0, 0]);
  expect(JSON.parse(String(fetchJson.mock.calls[1]?.[1].body))).toEqual({
    workflow: { version: 0.4 },
  });
  // Neither workflow records links of its own, so its Cubes are linked as any added Cube is.
  expect(order.filter((step) => step.startsWith('linked'))).toHaveLength(0);

  // A workflow the Python application saved records its links: they are handed over, even none.
  await service.import({ extra: { sugarcubes_composition: { value_relations: [] } } }, [0, 0]);
  expect(order.filter((step) => step.startsWith('linked'))).toEqual(['linked 0']);
});

test('a workflow from a file is reconciled before its own workflow opens, and placed there', async () => {
  const order: string[] = [];
  const reply = (plan: unknown) => ({
    response: { status: 200 },
    data: { valid: plan !== null, diagnostics: [], plan, exact_integers: [] },
  });
  let plan: unknown = { semantic_hash: 'a'.repeat(64), instances: [], connections: [] };
  const service = new LegacyWorkflowImportService(
    new LegacyWorkflowAuthoringApi({
      fetchJson: async () => {
        order.push('reconciled');
        return reply(plan);
      },
    } as unknown as CubeLibraryApi),
    {
      materializeWorkflow: (_plan: unknown, origin: readonly number[]) => {
        order.push('placed at ' + origin.join());
        return { semanticHash: 'h', instanceIds: [], warnings: [] };
      },
    } as unknown as NativeWorkflowMaterializer,
    () => undefined,
    () => undefined,
  );
  await service.open({ version: 0.4 }, undefined, async () => {
    order.push('own workflow opened');
    return [40, 60];
  });
  expect(order).toEqual(['reconciled', 'own workflow opened', 'placed at 40,60']);

  // A file that cannot be imported opens nothing.
  plan = null;
  order.length = 0;
  await expect(
    service.open({ version: 0.4 }, undefined, async () => {
      order.push('own workflow opened');
      return [0, 0];
    }),
  ).rejects.toThrow();
  expect(order).not.toContain('own workflow opened');
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
