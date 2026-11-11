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
/** Verify real DOM image ownership with controlled external decode and URL boundaries. */

import { jest } from '@jest/globals';
import {
  BrowserCanvasImageLoader,
  type BrowserCanvasImagePayload,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasImageLoader.js';
import type { CanvasImageInput } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasImageResource.js';
import { canvasImageInput, settleCanvasImages } from '../support/CanvasImageFixture.js';

/** Wrap a typed platform payload in an already admitted resource request. */
function input(payload: BrowserCanvasImagePayload): CanvasImageInput<BrowserCanvasImagePayload> {
  return { ...canvasImageInput('image'), preview: payload.kind === 'preview', payload };
}

describe('browser canvas image ownership', () => {
  let images: HTMLImageElement[];
  let revoke: ReturnType<typeof jest.fn<(url: string) => void>>;
  let loader: BrowserCanvasImageLoader;

  beforeEach(() => {
    images = [];
    revoke = jest.fn();
    const original = document.createElement.bind(document);
    jest.spyOn(document, 'createElement').mockImplementation((tag: string) => {
      const element = original(tag);
      if (element instanceof HTMLImageElement) images.push(element);
      return element;
    });
    loader = new BrowserCanvasImageLoader(document, { create: () => 'blob:canvas-owned', revoke });
  });
  afterEach(() => {
    loader.dispose();
    jest.restoreAllMocks();
  });

  /** Complete one intrinsic-size load without pretending jsdom implements a raster decoder. */
  function loaded(width = 3840, height = 2160): HTMLImageElement {
    const image = images.at(-1)!;
    Object.defineProperties(image, {
      naturalWidth: { value: width },
      naturalHeight: { value: height },
    });
    image.dispatchEvent(new Event('load'));
    return image;
  }

  test('releases displayed ordinary images without revoking host URLs', async () => {
    const load = loader.start(input({ kind: 'file', url: '/view?filename=final.png&type=output' }));
    const element = loaded();
    const frame = await load.completion;
    expect(frame.handle).toBe(element);
    expect(frame.dimensions).toEqual({ width: 3840, height: 2160 });
    document.body.append(element);
    frame.release();
    frame.release();
    expect(element.isConnected).toBe(false);
    expect(element.hasAttribute('src')).toBe(false);
    expect(loader.diagnostics()).toEqual({ elements: 0, previewUrls: 0 });
    expect(revoke).not.toHaveBeenCalled();
  });

  test('revokes each owned preview URL on cancellation and ignores later platform completion', async () => {
    const load = loader.start(input({ kind: 'preview', blob: new Blob(['preview']) }));
    const rejection = expect(load.completion).rejects.toThrow('canceled');
    expect(loader.diagnostics()).toEqual({ elements: 1, previewUrls: 1 });
    load.cancel();
    load.cancel();
    loaded();
    await rejection;
    expect(revoke).toHaveBeenCalledTimes(1);
    expect(loader.diagnostics()).toEqual({ elements: 0, previewUrls: 0 });
  });

  test('waits for decode and releases an unresolved decode after surface teardown', async () => {
    let finish: () => void = () => {};
    const decode = jest.fn(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    Object.defineProperty(HTMLImageElement.prototype, 'decode', {
      configurable: true,
      value: decode,
    });
    try {
      const load = loader.start(input({ kind: 'preview', blob: new Blob(['preview']) }));
      const rejection = expect(load.completion).rejects.toThrow('canceled');
      loaded();
      expect(loader.diagnostics().elements).toBe(1);
      loader.dispose();
      finish();
      await settleCanvasImages();
      expect(loader.diagnostics()).toEqual({ elements: 0, previewUrls: 0 });
      expect(revoke).toHaveBeenCalledTimes(1);
      load.cancel();
      await rejection;
    } finally {
      Reflect.deleteProperty(HTMLImageElement.prototype, 'decode');
    }
  });

  test('reports invalid dimensions and decode/network failures with all ownership released', async () => {
    const load = loader.start(input({ kind: 'preview', blob: new Blob(['preview']) }));
    const rejected = expect(load.completion).rejects.toThrow('dimensions');
    loaded(0, 0);
    await rejected;
    expect(loader.diagnostics()).toEqual({ elements: 0, previewUrls: 0 });
    const next = loader.start(input({ kind: 'file', url: '/view?filename=missing.png' }));
    const failed = expect(next.completion).rejects.toThrow('loading failed');
    images.at(-1)!.dispatchEvent(new Event('error'));
    await failed;
    expect(loader.diagnostics().elements).toBe(0);
  });

  test.each(['https://foreign.example/view', 'javascript:alert(1)', 'data:image/png;base64,AAA'])(
    'rejects unowned URL %s before platform loading',
    (url) => {
      expect(() => loader.start(input({ kind: 'file', url }))).toThrow('current host origin');
      expect(loader.diagnostics()).toEqual({ elements: 0, previewUrls: 0 });
    },
  );
});
