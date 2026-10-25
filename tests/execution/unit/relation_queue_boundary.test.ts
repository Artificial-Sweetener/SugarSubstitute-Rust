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
/** Prove relation preparation precedes both native Comfy and direct Cube queue paths. */
import { expect, test, jest } from '@jest/globals';
import { ComfyPromptQueueBridge } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptQueueBridge.js';

test.each([true, false])(
  'prepares relation values once before queue ownership is dispatched: %s',
  async (direct) => {
    const order: string[] = [],
      api = {
        queuePrompt: async (_position: number, payload: unknown) => {
          order.push('native');
          return payload;
        },
      };
    const bridge = new ComfyPromptQueueBridge({
      api,
      prepare: () => {
        order.push('relations');
        return { effective: 42 };
      },
      transform: async (payload) => {
        order.push('transform');
        return { payload };
      },
      executeDirect: async (_position, payload) => {
        order.push('direct');
        return payload;
      },
      shouldExecuteDirect: () => direct,
    });
    bridge.install();
    expect(await api.queuePrompt(0, {})).toEqual({ effective: 42 });
    expect(order).toEqual(direct ? ['relations', 'direct'] : ['relations', 'transform', 'native']);
    bridge.dispose();
  },
);

test('rejects invalid direct execution before consuming a shared seed', async () => {
  const prepare = jest.fn((payload: unknown) => payload);
  const api = { queuePrompt: async (_position: number, payload: unknown) => payload };
  const bridge = new ComfyPromptQueueBridge({
    api,
    prepare,
    preflight: () => {
      throw new Error('invalid nesting');
    },
    transform: async (payload) => ({ payload }),
    executeDirect: async (_position, payload) => payload,
  });
  bridge.install();
  await expect(api.queuePrompt(0, {})).rejects.toThrow('invalid nesting');
  expect(prepare).not.toHaveBeenCalled();
  bridge.dispose();
});
