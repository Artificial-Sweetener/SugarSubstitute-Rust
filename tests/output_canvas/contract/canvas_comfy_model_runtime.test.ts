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
/** Prove the canvas delegates rendering to Comfy and serializes its uncancellable loader. */
import { jest } from '@jest/globals';
import { ComfyCanvasModelRuntime } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasModelRuntime.js';

test('mounts Comfy viewers, captures through the host, and disposes exactly once', async () => {
  const raw = {
    updateStatusMouseOnViewer: jest.fn(),
    loadModel: jest.fn<() => Promise<void>>().mockResolvedValue(),
    getCurrentModel: () => ({}),
    handleResize: jest.fn(),
    captureThumbnail: async () => 'data:image/png;base64,AA==',
    getCameraState: () => ({ zoom: 2 }),
    setCameraState: jest.fn(),
    remove: jest.fn(),
  };
  const factory = jest.fn(() => raw);
  const runtime = new ComfyCanvasModelRuntime(() => ({ createLoad3d: { createLoad3d: factory } }));
  const element = document.createElement('div');
  const viewer = await runtime.load(
    element,
    '/view?filename=synthetic.glb',
    'synthetic.glb',
    new AbortController().signal,
  );
  expect(factory).toHaveBeenCalledWith(element, { isViewerMode: true, disablePreview: true });
  expect(raw.loadModel).toHaveBeenCalledWith('/view?filename=synthetic.glb', 'synthetic.glb');
  element.dispatchEvent(new Event('pointerenter'));
  expect(raw.updateStatusMouseOnViewer).toHaveBeenLastCalledWith(true);
  element.dispatchEvent(new Event('pointerleave'));
  expect(raw.updateStatusMouseOnViewer).toHaveBeenLastCalledWith(false);
  expect(await viewer.capture()).toBe('data:image/png;base64,AA==');
  viewer.resize();
  viewer.restore(viewer.camera());
  expect(raw.setCameraState).toHaveBeenCalledWith({ zoom: 2 });
  viewer.dispose();
  viewer.dispose();
  expect(raw.remove).toHaveBeenCalledTimes(1);
});

test('never allocates a canceled queued viewer and retires a canceled in-flight load', async () => {
  let finish: () => void = () => {};
  const raw = {
    loadModel: () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
    getCurrentModel: () => ({}),
    remove: jest.fn(),
  };
  const factory = jest.fn(() => raw);
  const runtime = new ComfyCanvasModelRuntime(() => ({ createLoad3d: { createLoad3d: factory } }));
  const a = new AbortController();
  const b = new AbortController();
  const first = runtime.load(document.createElement('div'), '/view', 'a.glb', a.signal);
  await Promise.resolve();
  const second = runtime.load(document.createElement('div'), '/view', 'b.glb', b.signal);
  const firstRejected = expect(first).rejects.toBeDefined();
  const secondRejected = expect(second).rejects.toBeDefined();
  a.abort();
  b.abort();
  finish();
  await firstRejected;
  await secondRejected;
  expect(factory).toHaveBeenCalledTimes(1);
  expect(raw.remove).toHaveBeenCalledTimes(1);
});

test('reports an unavailable host factory without constructing a substitute renderer', async () => {
  const runtime = new ComfyCanvasModelRuntime(() => ({}));
  await expect(
    runtime.load(
      document.createElement('div'),
      '/view',
      'synthetic.glb',
      new AbortController().signal,
    ),
  ).rejects.toThrow('ComfyUI');
});
