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
/** Prove native transaction ownership at the real compiled adapter's Photoshop boundary. */
import { jest } from '@jest/globals';
import { build } from 'esbuild';
import { runInNewContext } from 'node:vm';
import type {
  CanvasContext,
  CanvasInsertImage,
  CanvasInsertResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type { PreparedCanvasImage } from '../../../integrations/photoshop/src/application/InsertCanvasImage.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { BUILD_FIXTURE_SETUP_TIMEOUT_MS } from '../../frontend_core/support/BuildFixtureBudget.js';

interface NativeCanvas {
  context(): CanvasContext;
  insert(
    request: CanvasInsertImage,
    image: PreparedCanvasImage,
    cancel: CanvasCancellation,
  ): Promise<CanvasInsertResult>;
}
const material: PreparedCanvasImage = {
  key: 'pixels',
  width: 768,
  height: 512,
  dispose: async () => {},
};
let code = '';
beforeAll(async () => {
  const output = await build({
    entryPoints: ['integrations/photoshop/src/infrastructure/PhotoshopCanvasDocuments.ts'],
    bundle: true,
    write: false,
    format: 'cjs',
    platform: 'neutral',
    target: 'es2022',
    external: ['photoshop'],
  });
  code = output.outputFiles[0]!.text;
}, BUILD_FIXTURE_SETUP_TIMEOUT_MS);

/** Keep only the native document, imaging and history boundaries replaceable. */
function fixture() {
  const pixels = { width: 768, height: 512 };
  const first = { id: 9 };
  const move = jest
    .fn<(relative: unknown, position: string) => Promise<void>>()
    .mockResolvedValue(undefined);
  const layer = { id: 3, move };
  const create = jest
    .fn<(kind: string, options: { name: string }) => Promise<typeof layer | null>>()
    .mockResolvedValue(layer);
  const target = {
    id: 1,
    width: 512,
    height: 384,
    mode: 'RGB',
    bitsPerChannel: 'bitDepth8',
    colorProfileName: 'sRGB',
    pixelAspectRatio: 1,
    quickMaskMode: false,
    artboards: [] as number[],
    layers: [first],
    selection: { bounds: { left: 1, top: 2, right: 3, bottom: 4 } },
    createLayer: create,
  };
  const other = { ...target, id: 2 };
  const app = { activeDocument: target, documents: [target, other] };
  const suspend = jest.fn<() => Promise<{ id: number }>>().mockResolvedValue({ id: 1 });
  const resume = jest
    .fn<(history: unknown, commit: boolean) => Promise<void>>()
    .mockResolvedValue(undefined);
  const put = jest.fn<(options: unknown) => Promise<void>>().mockResolvedValue(undefined);
  const enterModal = jest
    .fn<
      (
        callback: (context: typeof execution) => Promise<void>,
        options: { commandName: string; timeOut: number },
      ) => Promise<void>
    >()
    .mockImplementation(async (callback) => callback(execution));
  const native = {
    app,
    core: {
      executeAsModal: enterModal,
    },
    imaging: { putPixels: put },
    constants: {
      DocumentMode: { RGB: 'RGB' },
      LayerKind: { NORMAL: 'normal' },
      ElementPlacement: { PLACEBEFORE: 'before' },
    },
  };
  const execution = {
    isCancelled: false,
    hostControl: { suspendHistory: suspend, resumeHistory: resume },
  };
  const module: { exports: Record<string, unknown> } = { exports: {} };
  runInNewContext(code, {
    module,
    require: (name: string): unknown => {
      if (name !== 'photoshop') throw new Error('Unexpected native dependency.');
      return native;
    },
  });
  const ctor = module.exports.PhotoshopCanvasDocuments;
  if (typeof ctor !== 'function') throw new Error('Missing native adapter.');
  // The compiled native module is intentionally isolated from this test's constructor identity.
  const make = ctor as new (assets: { imageData(key: string): typeof pixels }) => NativeCanvas;
  const imageData = jest.fn<(key: string) => typeof pixels>().mockReturnValue(pixels);
  const api = new make({ imageData });
  const request: CanvasInsertImage = {
    document: api.context().document!,
    image: { format: 'png', encoding: 'base64', data: 'prepared' },
    layerName: 'Generated',
    position: null,
  };
  return {
    api,
    request,
    cancel: new CanvasCancellation(),
    target,
    other,
    app,
    pixels,
    create,
    move,
    first,
    put,
    suspend,
    resume,
    imageData,
    execution,
    enterModal,
  };
}

test('creates a raster layer at final coordinates and commits one history transaction', async () => {
  const f = fixture();
  const selection = JSON.stringify(f.target.selection);
  expect(await f.api.insert(f.request, material, f.cancel)).toMatchObject({
    status: 'inserted',
    left: -128,
    top: -64,
    width: 768,
    height: 512,
  });
  expect(f.create).toHaveBeenCalledWith('normal', { name: 'Generated' });
  expect(f.move).toHaveBeenCalledWith(f.first, 'before');
  expect(f.put).toHaveBeenCalledWith({
    documentID: 1,
    layerID: 3,
    imageData: f.pixels,
    replace: true,
    targetBounds: { left: -128, top: -64, width: 768, height: 512 },
    commandName: 'Insert generated image',
  });
  expect(f.suspend).toHaveBeenCalledTimes(1);
  expect(f.suspend).toHaveBeenCalledWith({ documentID: 1, name: 'Insert generated image' });
  expect(f.resume).toHaveBeenCalledTimes(1);
  expect(f.resume).toHaveBeenCalledWith({ id: 1 }, true);
  expect(f.put.mock.invocationCallOrder[0]).toBeGreaterThan(f.suspend.mock.invocationCallOrder[0]!);
  expect(f.put.mock.invocationCallOrder[0]).toBeLessThan(f.resume.mock.invocationCallOrder[0]!);
  expect(JSON.stringify(f.target.selection)).toBe(selection);
  expect([f.target.width, f.target.height]).toEqual([512, 384]);
  expect(f.app.documents).toHaveLength(2);
});

test('writes a trimmed inpaint at its captured origin without changing the active document', async () => {
  const f = fixture();
  f.app.activeDocument = f.other;
  f.pixels.width = 31;
  f.pixels.height = 19;
  const result = await f.api.insert(
    { ...f.request, position: { left: 73, top: 119 } },
    { ...material, width: 31, height: 19 },
    f.cancel,
  );
  expect(result).toMatchObject({ status: 'inserted', left: 73, top: 119, width: 31, height: 19 });
  expect(f.put).toHaveBeenCalledWith(
    expect.objectContaining({
      documentID: 1,
      targetBounds: { left: 73, top: 119, width: 31, height: 19 },
    }),
  );
  expect(f.app.activeDocument).toBe(f.other);
});

test.each(['write', 'create', 'cancel'])(
  'rolls back the complete insertion on %s failure',
  async (failure) => {
    const f = fixture();
    if (failure === 'write') f.put.mockRejectedValue(new Error('Pixel write failed'));
    if (failure === 'create') f.create.mockResolvedValue(null);
    if (failure === 'cancel')
      f.put.mockImplementation(async () => {
        f.cancel.cancel();
      });
    if (failure === 'cancel')
      expect(await f.api.insert(f.request, material, f.cancel)).toMatchObject({
        status: 'cancelled',
      });
    else await expect(f.api.insert(f.request, material, f.cancel)).rejects.toThrow();
    expect(f.resume).toHaveBeenCalledWith({ id: 1 }, false);
    expect(f.resume).not.toHaveBeenCalledWith({ id: 1 }, true);
  },
);

test('preserves both errors if a failed insertion cannot roll back', async () => {
  const f = fixture();
  f.put.mockRejectedValue(new Error('Write failed'));
  f.resume.mockRejectedValue(new Error('Rollback failed'));
  await expect(f.api.insert(f.request, material, f.cancel)).rejects.toThrow(
    'insertion and rollback failed',
  );
});

test('refuses stale geometry and unavailable pixels before starting document history', async () => {
  const f = fixture();
  f.target.width = 1024;
  expect(await f.api.insert(f.request, material, f.cancel)).toMatchObject({ status: 'stale' });
  expect(f.create).not.toHaveBeenCalled();
  expect(f.suspend).not.toHaveBeenCalled();
  f.target.width = 512;
  f.pixels.width = 1;
  await expect(f.api.insert(f.request, material, f.cancel)).rejects.toThrow('admitted frame');
  expect(f.suspend).not.toHaveBeenCalled();
});

test.each(['mode', 'artboards', 'quick-mask'])(
  'refuses unsupported %s state without creating a layer',
  async (state) => {
    const f = fixture();
    if (state === 'mode') f.target.mode = 'GRAY';
    else if (state === 'artboards') f.target.artboards = [1];
    else f.target.quickMaskMode = true;
    const request = { ...f.request, document: f.api.context().document! };
    expect(await f.api.insert(request, material, f.cancel)).toMatchObject({
      status: 'unsupported',
    });
    expect(f.create).not.toHaveBeenCalled();
    expect(f.suspend).not.toHaveBeenCalled();
  },
);

test('bounds native modal admission to one second before inserting output', async () => {
  const f = fixture();
  await f.api.insert(f.request, material, f.cancel);
  expect(f.enterModal).toHaveBeenCalledWith(expect.any(Function), {
    commandName: 'Insert generated image',
    timeOut: 1,
  });
});

test('refuses a busy host before creating an output layer or history', async () => {
  const f = fixture();
  const busy = new Error('Photoshop modal admission expired');
  f.enterModal.mockRejectedValue(busy);
  await expect(f.api.insert(f.request, material, f.cancel)).rejects.toBe(busy);
  expect(f.create).not.toHaveBeenCalled();
  expect(f.put).not.toHaveBeenCalled();
  expect(f.suspend).not.toHaveBeenCalled();
});
