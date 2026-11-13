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
/** Prove direct Cube transport preserves extension queue observation and isolated submissions. */
import { expect, jest, test } from '@jest/globals';
import { ComfyPromptQueueBridge } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptQueueBridge.js';
import { ComfyDirectPromptTransport } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyDirectPromptTransport.js';
import type { ComfyPromptQueueApi } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptQueueBridge.js';
import type { ApiResponse } from '../../../frontend/comfyui/ui/types/graph.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

/** Model Comfy's public queue and an earlier extension's accepted-prompt observer. */
function fixture() {
  const accepted: unknown[] = [];
  const nativeFetch = jest.fn(async (_path: string, _options?: RequestInit) =>
    response({ native: true }),
  );
  const api: ComfyPromptQueueApi & {
    fetchApi(path: string, options?: RequestInit): Promise<ApiResponse>;
  } = {
    fetchApi: nativeFetch,
    async queuePrompt(position, payload, options) {
      if (!isRecord(payload)) throw new TypeError('Expected prompt payload');
      const result = await api.fetchApi('/prompt', {
        method: 'POST',
        body: JSON.stringify({
          prompt: payload.output,
          number: position,
          extra_data: { extra_pnginfo: { workflow: payload.workflow } },
        }),
      });
      const value = await result.json();
      accepted.push({ payload, value, options });
      return value;
    },
  };
  const transport = new ComfyDirectPromptTransport({
    api,
    response,
    nextId: (() => {
      let id = 0;
      return () => String(++id);
    })(),
  });
  const original = api.queuePrompt;
  if (!original) throw new Error('Missing queue fixture');
  const submit = (payload: unknown) => original.call(api, 0, payload);
  return { api, nativeFetch, accepted, transport, submit };
}

/** Supply the response capabilities used by Comfy at the external boundary. */
function response(value: unknown): ApiResponse {
  return { ok: true, status: 200, json: async () => value };
}

test('direct Cube runs traverse earlier queue observers exactly once without sending a native prompt', async () => {
  const f = fixture(),
    input = { output: {}, workflow: { id: 'anima', nodes: [] } };
  const options = { previewMethod: 'latent2rgb', partialExecutionTargets: ['sink'] };
  const execute = jest.fn(async () => ({ accepted: true, prompt_id: 'cube-run' }));
  const bridge = new ComfyPromptQueueBridge({
    api: f.api,
    transform: async (payload) => ({ payload }),
    executeDirect: (_position, payload, _forwarded, submit) =>
      f.transport.queue(payload, () => execute(), submit),
  });
  bridge.install();
  expect(await f.api.queuePrompt?.(-1, input, options)).toEqual({
    accepted: true,
    prompt_id: 'cube-run',
  });
  expect(execute).toHaveBeenCalledTimes(1);
  expect(f.accepted).toHaveLength(1);
  expect(f.accepted[0]).toMatchObject({ value: { prompt_id: 'cube-run' }, options });
  expect(f.nativeFetch).not.toHaveBeenCalled();
  expect(f.api.fetchApi).toBe(f.nativeFetch);
  expect(input.workflow).toEqual({ id: 'anima', nodes: [] });
  bridge.dispose();
});

test('overlapping direct requests and unrelated native requests retain their own results', async () => {
  const f = fixture();
  let release: (() => void) | undefined;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  const a = f.transport.queue(
    { output: {}, workflow: { id: 'a' } },
    async () => {
      await held;
      return { prompt_id: 'a' };
    },
    f.submit,
  );
  const b = f.transport.queue(
    { output: {}, workflow: { id: 'b' } },
    async () => ({
      prompt_id: 'b',
    }),
    f.submit,
  );
  expect(await b).toEqual({ prompt_id: 'b' });
  expect(await f.api.queuePrompt?.(0, { output: {}, workflow: { id: 'native' } })).toEqual({
    native: true,
  });
  release?.();
  expect(await a).toEqual({ prompt_id: 'a' });
  expect(f.nativeFetch).toHaveBeenCalledTimes(1);
  expect(f.api.fetchApi).toBe(f.nativeFetch);
  expect(f.accepted).toHaveLength(3);
});

test('backend failure preserves its cause, releases transport and creates no accepted prompt', async () => {
  const f = fixture(),
    failure = new Error('Cube validation failed');
  await expect(
    f.transport.queue(
      { output: {}, workflow: {} },
      async () => {
        throw failure;
      },
      f.submit,
    ),
  ).rejects.toBe(failure);
  expect(f.accepted).toEqual([]);
  expect(f.api.fetchApi).toBe(f.nativeFetch);
  expect(f.nativeFetch).not.toHaveBeenCalled();
});

test('an extension that bypasses the expected transport cannot silently submit a different prompt', async () => {
  const f = fixture();
  const api = { ...f.api, queuePrompt: async () => ({ prompt_id: 'unexpected' }) };
  const transport = new ComfyDirectPromptTransport({ api, response, nextId: () => 'bypassed' });
  await expect(
    transport.queue({ output: {}, workflow: {} }, async () => ({}), api.queuePrompt),
  ).rejects.toThrow('did not reach');
  expect(api.fetchApi).toBe(f.nativeFetch);
});

test('duplicate transport calls fail closed and never enqueue the Cube twice', async () => {
  const f = fixture(),
    execute = jest.fn(async () => ({ prompt_id: 'once' }));
  const api = {
    ...f.api,
    async queuePrompt(_position: number, payload: unknown) {
      if (!isRecord(payload)) throw new TypeError('Expected prompt payload');
      const options = {
        method: 'POST',
        body: JSON.stringify({ extra_data: { extra_pnginfo: { workflow: payload.workflow } } }),
      };
      await api.fetchApi('/prompt', options);
      return await api.fetchApi('/prompt', options);
    },
  };
  const transport = new ComfyDirectPromptTransport({ api, response, nextId: () => 'double' });
  await expect(
    transport.queue({ workflow: {} }, execute, (payload) => api.queuePrompt(0, payload)),
  ).rejects.toThrow('duplicate request');
  expect(execute).toHaveBeenCalledTimes(1);
  expect(f.nativeFetch).not.toHaveBeenCalled();
  expect(api.fetchApi).toBe(f.nativeFetch);
});

test('later fetch wrappers remain installed and retained adapters pass ordinary requests through', async () => {
  const f = fixture();
  let release: (() => void) | undefined;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  const pending = f.transport.queue(
    { workflow: {} },
    async () => {
      await held;
      return { prompt_id: 'first' };
    },
    f.submit,
  );
  const retained = f.api.fetchApi;
  const later = jest.fn(async (path: string, options?: RequestInit) =>
    retained.call(f.api, path, options),
  );
  f.api.fetchApi = later;
  release?.();
  await pending;
  expect(f.api.fetchApi).toBe(later);
  await f.api.fetchApi('/queue');
  expect(f.nativeFetch).toHaveBeenCalledWith('/queue', undefined);
  expect(
    await f.transport.queue({ workflow: {} }, async () => ({ prompt_id: 'next' }), f.submit),
  ).toEqual({ prompt_id: 'next' });
  expect(f.api.fetchApi).toBe(later);
});

test('queue wrappers installed after composition are captured at queue installation', async () => {
  const f = fixture(),
    laterObserved: unknown[] = [];
  const original = f.api.queuePrompt;
  if (!original) throw new Error('Missing queue fixture');
  f.api.queuePrompt = async (position, payload, options) => {
    const result = await original.call(f.api, position, payload, options);
    laterObserved.push(result);
    return result;
  };
  const bridge = new ComfyPromptQueueBridge({
    api: f.api,
    transform: async (payload) => ({ payload }),
    executeDirect: (_position, payload, _options, submit) =>
      f.transport.queue(payload, async () => ({ prompt_id: 'late-wrapper' }), submit),
  });
  bridge.install();
  await f.api.queuePrompt(0, { output: {}, workflow: {} });
  expect(laterObserved).toEqual([{ prompt_id: 'late-wrapper' }]);
  expect(f.accepted).toHaveLength(1);
  bridge.dispose();
});
