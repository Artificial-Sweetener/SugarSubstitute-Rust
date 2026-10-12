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
/** Prove submission metadata cannot alter queue outcomes or revive disposed integration state. */

import { jest } from '@jest/globals';
import {
  ComfyPromptQueueBridge,
  type ComfyPromptQueueApi,
  type PromptQueueObservation,
} from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptQueueBridge.js';

/** Create independently inspectable callbacks for one submission. */
function observation(): PromptQueueObservation {
  return { prepared: jest.fn(), accepted: jest.fn(), rejected: jest.fn(), cancelled: jest.fn() };
}

test.each(['capture', 'prepared', 'accepted', 'rejected'] as const)(
  'contains %s observation failure and preserves the original queue outcome',
  async (stage) => {
    const result = { prompt_id: 'host' };
    const failure = new Error('Host rejection');
    const metadataFailure = new Error('Metadata failure');
    const callbacks = observation();
    const failed = () => {
      throw metadataFailure;
    };
    if (stage !== 'capture') callbacks[stage] = failed;
    const api: ComfyPromptQueueApi = {
      queuePrompt: async () => {
        if (stage === 'rejected') throw failure;
        return result;
      },
    };
    const logger = { error: jest.fn() };
    const bridge = new ComfyPromptQueueBridge({
      api,
      transform: async (payload) => payload,
      observer: { logger, capture: stage === 'capture' ? failed : () => callbacks },
    });
    bridge.install();
    if (stage === 'rejected') await expect(api.queuePrompt?.(0, {})).rejects.toBe(failure);
    else expect(await api.queuePrompt?.(0, {})).toBe(result);
    expect(logger.error).toHaveBeenCalledWith(expect.any(String), {
      stage,
      error: metadataFailure,
    });
    if (stage === 'prepared') {
      expect(callbacks.cancelled).toHaveBeenCalledTimes(1);
      expect(callbacks.accepted).not.toHaveBeenCalled();
    }
    bridge.dispose();
  },
);

test('captures before transformation and suppresses accepted metadata after disposal while finishing the request', async () => {
  const order: string[] = [];
  let complete: ((result: unknown) => void) | undefined;
  const pending = new Promise<unknown>((resolve) => {
    complete = resolve;
  });
  const result = { prompt_id: 'host' };
  const callbacks = observation();
  const api: ComfyPromptQueueApi = { queuePrompt: async () => result };
  const bridge = new ComfyPromptQueueBridge({
    api,
    transform: () => {
      order.push('transform');
      return pending;
    },
    observer: {
      logger: { error: jest.fn() },
      capture: () => {
        order.push('capture');
        return callbacks;
      },
    },
  });
  bridge.install();
  const request = api.queuePrompt?.(0, {});
  expect(order).toEqual(['capture', 'transform']);
  bridge.dispose();
  bridge.dispose();
  if (!complete) throw new Error('Missing transform completion');
  complete({ output: {} });
  expect(await request).toBe(result);
  expect(callbacks.cancelled).toHaveBeenCalledTimes(1);
  expect(callbacks.prepared).not.toHaveBeenCalled();
  expect(callbacks.accepted).not.toHaveBeenCalled();
});
