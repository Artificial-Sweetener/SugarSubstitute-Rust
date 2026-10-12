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
/** Verify the sole SugarCubes queue adapter owns only host interception. */

import { jest } from '@jest/globals';
import { ComfyPromptQueueBridge } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptQueueBridge.js';
import type {
  ComfyPromptQueueApi,
  ComfyPromptQueueOptions,
} from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptQueueBridge.js';

test('transforms prompts and restores only its own queue wrapper', async () => {
  const queuePrompt = jest.fn(
    async (_position: number, payload: unknown, _options?: unknown) => payload,
  );
  const transform = jest.fn(async () => ({ output: { transformed: {} } }));
  const api = { queuePrompt };
  const bridge = new ComfyPromptQueueBridge({ api, transform });

  bridge.install();
  const options = { partialExecutionTargets: ['sink'] };
  await api.queuePrompt(2, { output: {} }, options);

  expect(transform).toHaveBeenCalledWith({ output: {} });
  expect(queuePrompt).toHaveBeenCalledWith(2, { output: { transformed: {} } }, options);

  bridge.dispose();
  expect(api.queuePrompt).toBe(queuePrompt);
});

test('rejects invalid nesting before transformation or host queue mutation', async () => {
  const queuePrompt = jest.fn(async (_position: number, payload: unknown) => payload);
  const transform = jest.fn(async (payload: unknown) => payload);
  const api = { queuePrompt };
  const bridge = new ComfyPromptQueueBridge({
    api,
    preflight: () => {
      throw new Error('Remove the nested SugarCube wrapper.');
    },
    transform,
    executeDirect: async () => ({}),
    shouldExecuteDirect: () => true,
  });
  bridge.install();

  await expect(api.queuePrompt(0, { output: {} })).rejects.toThrow(
    'Remove the nested SugarCube wrapper.',
  );
  expect(transform).not.toHaveBeenCalled();
  expect(queuePrompt).not.toHaveBeenCalled();
  bridge.dispose();
});

test('queues directly when SugarCubes owns the request and preserves legacy fallback', async () => {
  const queuePrompt = jest.fn(
    async (_position: number, payload: unknown, _options?: unknown) => payload,
  );
  const transform = jest.fn(async (payload: unknown) => payload);
  const executeDirect = jest.fn(async () => ({ prompt_id: 'direct' }));
  const api = { queuePrompt };
  const bridge = new ComfyPromptQueueBridge({
    api,
    transform,
    executeDirect,
    shouldExecuteDirect: (payload) => payload !== 'substitute',
  });
  bridge.install();

  const options = { previewMethod: 'latent2rgb' };
  await expect(api.queuePrompt(0, { output: {} }, options)).resolves.toEqual({
    prompt_id: 'direct',
  });
  await api.queuePrompt(0, 'substitute');

  expect(executeDirect).toHaveBeenCalledTimes(1);
  expect(executeDirect).toHaveBeenCalledWith(0, { output: {} }, options);
  expect(transform).toHaveBeenCalledWith('substitute');
  expect(queuePrompt).toHaveBeenCalledWith(0, 'substitute', undefined);
  bridge.dispose();
});

test('preserves ordinary callback order, host receiver, options and response identity', async () => {
  const calls: string[] = [];
  const input = { output: {} };
  const prepared = { output: { prepared: {} } };
  const transformed = { output: { transformed: {} } };
  const response = { prompt_id: 'ordinary-prompt' };
  const options: ComfyPromptQueueOptions = {
    partialExecutionTargets: ['nested:output'],
    previewMethod: 'latent2rgb',
  };
  const api: ComfyPromptQueueApi = {
    async queuePrompt(this: ComfyPromptQueueApi, position, payload, forwardedOptions) {
      calls.push('host');
      expect(this).toBe(api);
      expect(position).toBe(-1);
      expect(payload).toBe(transformed);
      expect(forwardedOptions).toBe(options);
      return response;
    },
  };
  const bridge = new ComfyPromptQueueBridge({
    api,
    prepare: (payload) => {
      calls.push('prepare');
      expect(payload).toBe(input);
      return prepared;
    },
    transform: async (payload) => {
      calls.push('transform');
      expect(payload).toBe(prepared);
      return transformed;
    },
    executeDirect: async () => {
      throw new Error('Ordinary submissions must use the host queue.');
    },
    shouldExecuteDirect: (payload) => {
      calls.push('decision');
      expect(payload).toBe(input);
      return false;
    },
    preflight: () => calls.push('preflight'),
  });
  bridge.install();

  expect(await api.queuePrompt?.(-1, input, options)).toBe(response);
  expect(calls).toEqual(['decision', 'prepare', 'transform', 'host']);
  bridge.dispose();
});

test('direct submission decides on original input and preflights before preparation', async () => {
  const calls: string[] = [];
  const input = { workflow: { id: 'workflow-a' } };
  const prepared = { output: {} };
  const response = { prompt_id: 'direct-prompt' };
  const options = { partialExecutionTargets: ['output'] };
  const queuePrompt = jest.fn(async () => response);
  const api: ComfyPromptQueueApi = { queuePrompt };
  const bridge = new ComfyPromptQueueBridge({
    api,
    shouldExecuteDirect: (payload) => {
      calls.push('decision');
      expect(payload).toBe(input);
      return true;
    },
    preflight: () => calls.push('preflight'),
    prepare: () => {
      calls.push('prepare');
      return prepared;
    },
    transform: async () => {
      throw new Error('Direct submissions must bypass transformation.');
    },
    executeDirect: async (position, payload, forwardedOptions) => {
      calls.push('direct');
      expect(position).toBe(3);
      expect(payload).toBe(prepared);
      expect(forwardedOptions).toBe(options);
      return response;
    },
  });
  bridge.install();

  expect(await api.queuePrompt?.(3, input, options)).toBe(response);
  expect(calls).toEqual(['decision', 'preflight', 'prepare', 'direct']);
  expect(queuePrompt).not.toHaveBeenCalled();
  bridge.dispose();
});

test.each(['prepare', 'transform', 'host', 'direct'] as const)(
  'preserves the original %s failure and prevents downstream queue calls',
  async (failureStage) => {
    const failure = new Error(`${failureStage} rejected`);
    const calls: string[] = [];
    const api: ComfyPromptQueueApi = {
      queuePrompt: async () => {
        calls.push('host');
        throw failure;
      },
    };
    const bridge = new ComfyPromptQueueBridge({
      api,
      prepare: (payload) => {
        calls.push('prepare');
        if (failureStage === 'prepare') throw failure;
        return payload;
      },
      transform: async (payload) => {
        calls.push('transform');
        if (failureStage === 'transform') throw failure;
        return payload;
      },
      executeDirect: async () => {
        calls.push('direct');
        throw failure;
      },
      shouldExecuteDirect: () => failureStage === 'direct',
    });
    bridge.install();

    await expect(api.queuePrompt?.(0, {})).rejects.toBe(failure);
    expect(calls).toEqual(
      failureStage === 'prepare'
        ? ['prepare']
        : failureStage === 'transform'
          ? ['prepare', 'transform']
          : failureStage === 'host'
            ? ['prepare', 'transform', 'host']
            : ['prepare', 'direct'],
    );
    bridge.dispose();
  },
);

test('an already started submission completes through the captured host after disposal', async () => {
  const pending = pendingTransformation();
  const response = { prompt_id: 'pending-prompt' };
  const queuePrompt = jest.fn(async (_position: number, _payload: unknown) => response);
  const api: ComfyPromptQueueApi = { queuePrompt };
  const bridge = new ComfyPromptQueueBridge({ api, transform: () => pending.promise });
  bridge.install();
  const request = api.queuePrompt?.(4, { workflow: { id: 'workflow-a' } });

  bridge.dispose();
  expect(api.queuePrompt).toBe(queuePrompt);
  pending.resolve({ output: { prepared: {} } });

  expect(await request).toBe(response);
  expect(queuePrompt).toHaveBeenCalledWith(4, { output: { prepared: {} } }, undefined);
});

test('repeated installation is stable and disposal preserves a later extension wrapper', () => {
  const original = jest.fn(async () => ({}));
  const api: ComfyPromptQueueApi = { queuePrompt: original };
  const bridge = new ComfyPromptQueueBridge({ api, transform: async (payload) => payload });
  bridge.install();
  const installed = api.queuePrompt;
  bridge.install();
  expect(api.queuePrompt).toBe(installed);

  const replacement = jest.fn(async () => ({ prompt_id: 'other-extension' }));
  api.queuePrompt = replacement;
  bridge.dispose();
  bridge.dispose();
  expect(api.queuePrompt).toBe(replacement);
});

test('installation without a host queue fails without creating a replacement', () => {
  const api: ComfyPromptQueueApi = {};
  const bridge = new ComfyPromptQueueBridge({ api, transform: async (payload) => payload });
  expect(() => bridge.install()).toThrow('Comfy queuePrompt is unavailable.');
  expect(api.queuePrompt).toBeUndefined();
  bridge.dispose();
});

test('a retained later wrapper still delegates after disposal and a fresh installation', async () => {
  const original = jest.fn(async () => ({ prompt_id: 'original' }));
  const transform = jest.fn(async (payload: unknown) => payload);
  const api: ComfyPromptQueueApi = { queuePrompt: original };
  const bridge = new ComfyPromptQueueBridge({ api, transform });
  bridge.install();
  const retained = api.queuePrompt;
  if (!retained) throw new Error('Missing installed queue');
  const later = async (position: number, payload: unknown, options?: ComfyPromptQueueOptions) =>
    await retained.call(api, position, payload, options);
  api.queuePrompt = later;
  bridge.dispose();
  await expect(api.queuePrompt(0, {})).resolves.toEqual({ prompt_id: 'original' });
  expect(transform).not.toHaveBeenCalled();
  bridge.install();
  await expect(api.queuePrompt(0, {})).resolves.toEqual({ prompt_id: 'original' });
  expect(transform).toHaveBeenCalledTimes(1);
  expect(original).toHaveBeenCalledTimes(2);
  bridge.dispose();
  expect(api.queuePrompt).toBe(later);
});

/** Hold transformation at its external async boundary until a lifecycle transition settles. */
function pendingTransformation(): { promise: Promise<unknown>; resolve(value: unknown): void } {
  let complete: ((value: unknown) => void) | undefined;
  const promise = new Promise<unknown>((resolve) => {
    complete = resolve;
  });
  return {
    promise,
    resolve(value) {
      if (!complete) throw new Error('The pending transformation has not been initialized.');
      complete(value);
    },
  };
}
