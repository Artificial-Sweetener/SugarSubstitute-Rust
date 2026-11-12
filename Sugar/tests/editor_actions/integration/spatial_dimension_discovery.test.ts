/** @jest-environment node */
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
/** Prove explicit discovery reuses compiled graph preparation and its bounded backend transport. */
import { jest } from '@jest/globals';
import { ComfySpatialProbeClient } from '../../../frontend/comfyui/ui/editor_actions/infrastructure/ComfySpatialProbeClient.js';
import { ComfyDimensionDiscovery } from '../../../frontend/comfyui/ui/editor_actions/infrastructure/ComfyDimensionDiscovery.js';
import type { DimensionBinding } from '../../../frontend/comfyui/ui/editor_actions/application/CanvasDimensionFields.js';
import type { ApiResponse } from '../../../frontend/comfyui/ui/types/graph.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';

const token = '00000000-0000-4000-8000-000000000001';
const base = '/sugarcubes/v2/executions/spatial';

/** Supply actual field addresses while keeping field mutation outside discovery. */
function fields(): DimensionBinding {
  return {
    width: {
      read: () => 1024,
      write: () => {
        throw new Error('Discovery must not write');
      },
      disabled: () => false,
      targets: () => [{ nodeId: '42:1', inputName: 'width' }],
    },
    height: {
      read: () => 768,
      write: () => {
        throw new Error('Discovery must not write');
      },
      disabled: () => false,
      targets: () => [{ nodeId: '42:1', inputName: 'height' }],
    },
  };
}

/** Retain the exact admitted request without replacing any graph or command policy. */
function transport() {
  const responses: unknown[] = [
    { token, outputs: [{ id: '7/images', label: '<Final image>' }] },
    { scale: 1.5 },
  ];
  const fetch = jest
    .fn<(path: string, options?: RequestInit) => Promise<ApiResponse>>()
    .mockImplementation(async () => ({ ok: true, json: async () => responses.shift() }));
  const report = jest.fn<(error: unknown) => void>();
  return { responses, fetch, report, client: new ComfySpatialProbeClient(fetch, report) };
}

test('discovery compiles once through normal preparation and never queues or mutates the workflow', async () => {
  const io = transport();
  const output = { '42:1': { class_type: 'EmptyImage', inputs: { width: 1024, height: 768 } } };
  const original = structuredClone(output);
  const graphToPrompt = jest
    .fn<() => Promise<unknown>>()
    .mockResolvedValue({ output, workflow: { nodes: [] } });
  const prepare = jest
    .fn<(payload: unknown) => Promise<unknown>>()
    .mockImplementation(async (payload) => payload);
  const queuePrompt = jest.fn();
  const discovery = new ComfyDimensionDiscovery({ graphToPrompt, queuePrompt }, prepare, io.client);
  expect(io.fetch).not.toHaveBeenCalled();
  const probe = await discovery.prepare(fields());
  expect(graphToPrompt).toHaveBeenCalledTimes(1);
  expect(prepare).toHaveBeenCalledTimes(1);
  expect(io.fetch).toHaveBeenCalledTimes(1);
  const options = io.fetch.mock.calls[0]?.[1];
  expect(JSON.parse(String(options?.body))).toEqual({
    prompt: output,
    width_targets: [{ nodeId: '42:1', inputName: 'width' }],
    height_targets: [{ nodeId: '42:1', inputName: 'height' }],
  });
  expect(await probe.discover('7/images')).toBe(1.5);
  expect(queuePrompt).not.toHaveBeenCalled();
  expect(output).toEqual(original);
  probe.dispose();
  probe.dispose();
  expect(io.fetch.mock.calls.map(([path]) => path)).toEqual([
    base + '/prepare',
    base + '/' + token + '/measure',
    base + '/' + token,
  ]);
  expect(io.fetch.mock.calls[2]?.[1]?.method).toBe('DELETE');
  await expect(probe.discover('7/images')).rejects.toThrow('unavailable');
});

test('workflow replacement during compilation is rejected before graph admission', async () => {
  const io = transport();
  let current = true;
  const discovery = new ComfyDimensionDiscovery(
    { graphToPrompt: async () => ({ output: {} }) },
    async (payload) => {
      current = false;
      return payload;
    },
    io.client,
  );
  await expect(discovery.prepare({ ...fields(), current: () => current })).rejects.toThrow(
    'cancelled',
  );
  expect(io.fetch).not.toHaveBeenCalled();
});

test('invalid backend output releases an admitted handle without measuring anything', async () => {
  const io = transport();
  io.responses.splice(0, 1, { token, outputs: [{ id: '', label: 'Invalid' }] });
  await expect(io.client.prepare({}, [], [])).rejects.toThrow('invalid probe output');
  expect(io.fetch.mock.calls[1]?.[0]).toBe(base + '/' + token);
  expect(io.fetch.mock.calls[1]?.[1]?.method).toBe('DELETE');
});

test('caller cancellation aborts native measurement observation and disposal releases its snapshot', async () => {
  const io = transport();
  const probe = await io.client.prepare({}, [], []);
  io.fetch.mockImplementation(async (_path, options) => {
    if (options?.method === 'DELETE') return { ok: true, json: async () => null };
    return await new Promise<ApiResponse>((_resolve, reject) =>
      options?.signal?.addEventListener('abort', () => reject(new Error('Aborted')), {
        once: true,
      }),
    );
  });
  const cancellation = new CanvasCancellation();
  const pending = probe.discover('7/images', cancellation);
  cancellation.cancel();
  await expect(pending).rejects.toThrow('Aborted');
  probe.dispose();
  expect(io.fetch.mock.calls[2]?.[1]?.method).toBe('DELETE');
});

test.each([0, -1, NaN, Infinity, 1001, '2'])(
  'invalid measured factor %s cannot reach sizing',
  async (scale) => {
    const io = transport();
    io.responses[1] = { scale };
    const probe = await io.client.prepare({}, [], []);
    await expect(probe.discover('7/images')).rejects.toThrow('valid measured scale');
    probe.dispose();
  },
);
