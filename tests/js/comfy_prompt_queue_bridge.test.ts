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
import { ComfyPromptQueueBridge } from '../../frontend/comfyui/ui/cube/execution/ComfyPromptQueueBridge.js';

test('transforms prompts and restores only its own queue wrapper', async () => {
  const queuePrompt = jest.fn(async (_position: number, payload: unknown) => payload);
  const transform = jest.fn(async () => ({ output: { transformed: {} } }));
  const api = { queuePrompt };
  const bridge = new ComfyPromptQueueBridge({ api, transform });

  bridge.install();
  await api.queuePrompt(2, { output: {} });

  expect(transform).toHaveBeenCalledWith({ output: {} });
  expect(queuePrompt).toHaveBeenCalledWith(2, { output: { transformed: {} } });

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
  });
  bridge.install();

  await expect(api.queuePrompt(0, { output: {} })).rejects.toThrow(
    'Remove the nested SugarCube wrapper.',
  );
  expect(transform).not.toHaveBeenCalled();
  expect(queuePrompt).not.toHaveBeenCalled();
  bridge.dispose();
});
