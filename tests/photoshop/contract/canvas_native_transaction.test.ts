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
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import type {
  CanvasContext,
  CanvasInsertImage,
  CanvasInsertResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type { PreparedCanvasImage } from '../../../integrations/photoshop/src/application/InsertCanvasImage.js';
import type { CanvasCancellationSource } from '../../../integrations/contracts/CanvasHost.js';
interface NativeCanvas {
  context(): CanvasContext;
  insert(
    request: CanvasInsertImage,
    image: PreparedCanvasImage,
    cancel: CanvasCancellationSource,
  ): Promise<CanvasInsertResult>;
}
const material: PreparedCanvasImage = {
  key: 'owned-source',
  width: 768,
  height: 512,
  dispose: async () => {},
};
/** Bundle the actual adapter once, keeping only Photoshop's unavailable native module external. */
async function source(): Promise<string> {
  const output = await build({
    entryPoints: ['integrations/photoshop/src/infrastructure/PhotoshopCanvasDocuments.ts'],
    bundle: true,
    write: false,
    format: 'cjs',
    platform: 'neutral',
    target: 'es2022',
    external: ['photoshop'],
  });
  const file = output.outputFiles[0];
  if (!file) throw new Error('Missing native transaction bundle.');
  return file.text;
}
let code = '';
beforeAll(async () => {
  code = await source();
});
/** Model document identity and host commands without replacing application or transaction logic. */
function fixture() {
  const closeTarget = jest.fn<() => void>(),
    closeSource = jest.fn<() => void>();
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
    closeWithoutSaving: closeTarget,
  };
  const sourceLayer = {
    isBackgroundLayer: false,
    allLocked: false,
    translate: jest.fn<(x: number, y: number) => Promise<void>>().mockResolvedValue(undefined),
    duplicate: jest.fn<() => Promise<{ id: number }>>().mockResolvedValue({ id: 3 }),
  };
  const opened = {
    ...target,
    id: 2,
    width: 768,
    height: 512,
    layers: [sourceLayer],
    closeWithoutSaving: closeSource,
  };
  const app = { activeDocument: target, documents: [target] };
  const suspend = jest.fn<() => Promise<{ id: number }>>().mockResolvedValue({ id: 1 });
  const resume = jest
    .fn<(history: unknown, commit: boolean) => Promise<void>>()
    .mockResolvedValue(undefined);
  const autoClose = jest.fn<(id: number) => Promise<void>>().mockResolvedValue(undefined);
  const execution = {
    isCancelled: false,
    hostControl: {
      suspendHistory: suspend,
      resumeHistory: resume,
      registerAutoCloseDocument: autoClose,
    },
  };
  const open = jest.fn<() => Promise<Record<string, unknown>[]>>().mockImplementation(async () => {
    app.activeDocument = opened;
    app.documents.push(opened);
    return [{}];
  });
  const host = {
    app,
    core: {
      executeAsModal: async (callback: (context: typeof execution) => Promise<void>) =>
        callback(execution),
    },
    action: { batchPlay: open },
    constants: {
      DocumentMode: { RGB: 'RGB' },
      ElementPlacement: { PLACEATBEGINNING: 'beginning' },
    },
  };
  const module: { exports: Record<string, unknown> } = { exports: {} };
  runInNewContext(code, {
    module,
    require: (name: string): unknown => {
      if (name !== 'photoshop') throw new Error('Unexpected native module.');
      return host;
    },
  });
  const ctor = module.exports.PhotoshopCanvasDocuments;
  if (typeof ctor !== 'function') throw new Error('Missing compiled native adapter.');
  // The VM's native module boundary has no shared JavaScript constructor identity.
  const create = ctor as new (
    assets: { token(key: string): string },
    report: (error: unknown) => void,
  ) => NativeCanvas;
  const report = jest.fn<(error: unknown) => void>();
  const token = jest.fn<(key: string) => string>().mockReturnValue('owned-session-token');
  const api = new create({ token }, report);
  const context = api.context();
  if (!context.document) throw new Error('Missing destination fixture.');
  const request: CanvasInsertImage = {
    document: context.document,
    image: { format: 'png', encoding: 'base64', data: 'prepared-by-assets' },
    layerName: 'Generated',
    position: null,
  };
  return {
    api,
    request,
    cancel: new CanvasCancellation(),
    target,
    opened,
    app,
    open,
    sourceLayer,
    suspend,
    resume,
    autoClose,
    closeTarget,
    closeSource,
    report,
    token,
  };
}
test('positions the source before duplication and commits one history step while closing only its owned source', async () => {
  const f = fixture();
  expect(await f.api.insert(f.request, material, f.cancel)).toMatchObject({
    status: 'inserted',
    left: -128,
    top: -64,
    width: 768,
    height: 512,
  });
  expect(f.sourceLayer.translate).toHaveBeenCalledWith(-128, -64);
  expect(f.sourceLayer.translate.mock.invocationCallOrder[0]).toBeLessThan(
    f.sourceLayer.duplicate.mock.invocationCallOrder[0]!,
  );
  expect(f.sourceLayer.duplicate).toHaveBeenCalledWith(f.target, 'beginning', 'Generated');
  expect(f.autoClose).toHaveBeenCalledWith(2);
  expect(f.resume).toHaveBeenCalledWith({ id: 1 }, true);
  expect(f.closeSource).toHaveBeenCalledTimes(1);
  expect(f.closeTarget).not.toHaveBeenCalled();
  expect(f.app.activeDocument).toBe(f.target);
  expect(f.open).toHaveBeenCalledWith(
    [
      {
        _obj: 'open',
        null: { _path: 'owned-session-token', _kind: 'local' },
        _options: { dialogOptions: 'dontDisplay' },
      },
    ],
    {},
  );
});
test.each(['native-error', 'no-document'])(
  'never closes an existing document when image open fails: %s',
  async (failure) => {
    const f = fixture();
    f.open.mockResolvedValue(failure === 'native-error' ? [{ _obj: 'error' }] : [{}]);
    await expect(f.api.insert(f.request, material, f.cancel)).rejects.toThrow();
    expect(f.closeTarget).not.toHaveBeenCalled();
    expect(f.closeSource).not.toHaveBeenCalled();
    expect(f.sourceLayer.duplicate).not.toHaveBeenCalled();
    expect(f.resume).toHaveBeenCalledWith({ id: 1 }, false);
  },
);
test('translates a captured patch origin directly instead of centering its trimmed pixels', async () => {
  const f = fixture();
  const position = { left: 73, top: 119 };
  f.opened.width = 31;
  f.opened.height = 19;
  expect(
    await f.api.insert(
      { ...f.request, position },
      { ...material, width: 31, height: 19 },
      f.cancel,
    ),
  ).toMatchObject({
    status: 'inserted',
    left: 73,
    top: 119,
    width: 31,
    height: 19,
  });
  expect(f.sourceLayer.translate).toHaveBeenCalledWith(73, 119);
  expect([f.target.width, f.target.height]).toEqual([512, 384]);
  expect(f.resume).toHaveBeenCalledWith({ id: 1 }, true);
});
test('refuses closed or resized captured destinations without opening image material', async () => {
  const f = fixture();
  f.target.width = 1024;
  expect(await f.api.insert(f.request, material, f.cancel)).toMatchObject({ status: 'stale' });
  expect(f.open).not.toHaveBeenCalled();
  expect(f.suspend).not.toHaveBeenCalled();
  f.app.documents = [];
  expect(await f.api.insert(f.request, material, f.cancel)).toMatchObject({ status: 'stale' });
});
test.each(['mode', 'artboards', 'quick-mask'])(
  'returns explicit unsupported destination state before mutation: %s',
  async (state) => {
    const f = fixture();
    if (state === 'mode') f.target.mode = 'GRAY';
    else if (state === 'artboards') f.target.artboards = [1];
    else f.target.quickMaskMode = true;
    const request = { ...f.request, document: f.api.context().document! };
    expect(await f.api.insert(request, material, f.cancel)).toMatchObject({
      status: 'unsupported',
    });
    expect(f.open).not.toHaveBeenCalled();
    expect(f.suspend).not.toHaveBeenCalled();
  },
);
test('rolls back a cancelled partial insertion and cleans its source without closing the destination', async () => {
  const f = fixture();
  f.sourceLayer.duplicate.mockImplementation(async () => {
    f.cancel.cancel();
    return { id: 3 };
  });
  expect(await f.api.insert(f.request, material, f.cancel)).toMatchObject({ status: 'cancelled' });
  expect(f.resume).toHaveBeenCalledWith({ id: 1 }, false);
  expect(f.closeSource).toHaveBeenCalledTimes(1);
  expect(f.closeTarget).not.toHaveBeenCalled();
});
