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
import { ComfyPromptQueueBridge } from '../../../frontend/comfyui/ui/cube/execution/ComfyPromptQueueBridge.js';

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
