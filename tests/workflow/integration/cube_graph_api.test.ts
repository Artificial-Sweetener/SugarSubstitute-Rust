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
/** Prove canonical analysis and reorder use bounded requests and actionable backend failures. */
import { expect, jest, test } from '@jest/globals';
import { CubeGraphApi } from '../../../frontend/comfyui/ui/workflow/infrastructure/CubeGraphApi.js';
import { graphResponse } from '../support/CubeGraphFixtures.js';

test('sends the schema-one serialized snapshot and exact requested permutation', async () => {
  const fetchJson = jest.fn(async (_url: string, _options?: RequestInit) => ({
    response: { status: 200 },
    data: graphResponse(),
  }));
  const api = new CubeGraphApi({ fetchJson });
  const snapshot = { revision: 'revision', serializedWorkflow: JSON.stringify({ nodes: [] }) };
  await api.analyze(snapshot);
  await api.reorder(snapshot, ['first', 'second'], ['second', 'first']);
  const [route, options] = fetchJson.mock.calls[1]!;
  expect(route).toBe('/sugarcubes/v2/workflows/reorder');
  expect(JSON.parse(String(options?.body))).toEqual({
    schema_version: 1,
    workflow: { nodes: [] },
    segment_instance_ids: ['first', 'second'],
    ordered_instance_ids: ['second', 'first'],
  });
  expect(options?.signal).toBeInstanceOf(AbortSignal);
});

test('preserves a canonical rejection message', async () => {
  const api = new CubeGraphApi({
    fetchJson: async () => ({
      response: { status: 422 },
      data: { error: { message: 'Fixed by explicit wiring.' } },
    }),
  });
  await expect(api.analyze({ revision: 'x', serializedWorkflow: '{}' })).rejects.toThrow(
    'Fixed by explicit wiring.',
  );
});

test('posts one full cart with exact captured documents and frames, excluding local artifact handles', async () => {
  const fetchJson = jest.fn(async (_url: string, _options?: RequestInit) => ({
    response: { status: 200 },
    data: graphResponse(),
  }));
  const api = new CubeGraphApi({ fetchJson });
  const snapshot = { revision: 'revision', serializedWorkflow: '{"nodes":[]}' };
  const document = { cube_id: 'captured', version: '1', implementation: { nodes: {} } };
  await api.applyStack(snapshot, [
    { source: 'existing', instanceId: 'first' },
    {
      source: 'new',
      instanceId: 'new-instance',
      catalogKey: 'local-handle',
      alias: 'Captured Cube',
      document,
      size: [920, 600],
    },
  ]);
  expect(fetchJson.mock.calls[0]?.[0]).toBe('/sugarcubes/v2/workflows/stack');
  expect(JSON.parse(String(fetchJson.mock.calls[0]?.[1]?.body))).toEqual({
    schema_version: 1,
    workflow: { nodes: [] },
    entries: [
      { source: 'existing', instance_id: 'first' },
      {
        source: 'new',
        instance_id: 'new-instance',
        alias: 'Captured Cube',
        document,
        size: [920, 600],
      },
    ],
  });
});

test('rejects corrupt snapshot input before making a request', async () => {
  const fetchJson = jest.fn(async () => ({ response: { status: 200 }, data: graphResponse() }));
  const api = new CubeGraphApi({ fetchJson });
  await expect(api.analyze({ revision: 'x', serializedWorkflow: '[]' })).rejects.toThrow();
  expect(fetchJson).not.toHaveBeenCalled();
});

test('rejects a late response even if the host transport does not honor cancellation', async () => {
  jest.useFakeTimers();
  try {
    const fetchJson = jest.fn(async (_url: string, _options?: RequestInit) => {
      await new Promise<void>((resolve) => globalThis.setTimeout(resolve, 15001));
      return { response: { status: 200 }, data: graphResponse() };
    });
    const api = new CubeGraphApi({ fetchJson });
    const request = api.analyze({ revision: 'x', serializedWorkflow: '{}' });
    const assertion = expect(request).rejects.toThrow('timed out');
    await jest.advanceTimersByTimeAsync(15001);
    await assertion;
    expect(fetchJson.mock.calls[0]?.[1]?.signal?.aborted).toBe(true);
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    jest.useRealTimers();
  }
});
