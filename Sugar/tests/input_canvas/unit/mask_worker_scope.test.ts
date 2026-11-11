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
/** Verify worker entry discovery cannot attach processing transport to Comfy's main page. */
import { jest } from '@jest/globals';
import {
  encodeCanvasRaster,
  decodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';

test('extension discovery in the page leaves host messages untouched', async () => {
  const listen = jest.spyOn(window, 'addEventListener');
  const post = jest.spyOn(window, 'postMessage').mockImplementation(() => {});
  try {
    await import('../../../frontend/comfyui/ui/input_canvas/infrastructure/MaskEditingWorker.js');
    expect(listen.mock.calls.filter(([type]) => type === 'message')).toHaveLength(0);
    window.dispatchEvent(
      new MessageEvent('message', {
        data: { protocol: 'sugarcubes.canvas', requestId: 1, type: 'context.reply' },
      }),
    );
    expect(post).not.toHaveBeenCalled();
  } finally {
    listen.mockRestore();
    post.mockRestore();
  }
});

test('the dedicated transport returns real edited coverage and attributes malformed requests', async () => {
  const { installMaskEditingWorker } = await import(
    '../../../frontend/comfyui/ui/input_canvas/infrastructure/MaskEditingWorker.js'
  );
  const listeners: ((event: MessageEvent<unknown>) => void)[] = [];
  const postMessage = jest.fn<(value: unknown) => void>();
  installMaskEditingWorker({
    addEventListener: (_type, listener) => {
      listeners.push(listener);
    },
    postMessage,
  });
  const source = encodeCanvasRaster(3, 2, 'coverage8', Uint8Array.from([0, 64, 0, 0, 255, 0]));
  listeners[0]!(
    new MessageEvent('message', {
      data: {
        id: 7,
        source,
        settings: { mask_dilation: 0, crop_factor: 1, post_dilation: 0, feather: 0 },
      },
    }),
  );
  const response = postMessage.mock.calls[0]?.[0];
  expect(response).toMatchObject({
    id: 7,
    result: {
      bounds: { left: 1, top: 0, width: 1, height: 2 },
      mask: { width: 1, height: 2, format: 'coverage8' },
    },
  });
  if (!response || typeof response !== 'object' || !('result' in response))
    throw new Error('Missing worker result.');
  const result = response.result;
  if (!result || typeof result !== 'object' || !('mask' in result))
    throw new Error('Missing mask.');
  expect(result.mask).toEqual(encodeCanvasRaster(1, 2, 'coverage8', Uint8Array.from([64, 255])));
  expect([...decodeCanvasRaster(source)]).toEqual([0, 64, 0, 0, 255, 0]);
  listeners[0]!(new MessageEvent('message', { data: { id: 8, source: {}, settings: {} } }));
  expect(postMessage.mock.calls[1]?.[0]).toMatchObject({
    id: 8,
    error: 'Invalid mask preparation request.',
  });
});
