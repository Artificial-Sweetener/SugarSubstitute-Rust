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
/** Prove native geometry writes use the captured document, canvas bounds and one undo transaction. */
import { jest } from '@jest/globals';
import { build } from 'esbuild';
import { runInNewContext } from 'node:vm';
import type { CanvasGeometryHost } from '../../../integrations/contracts/CanvasGeometry.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';

let code = '';
beforeAll(async () => {
  const output = await build({
    entryPoints: ['integrations/photoshop/src/infrastructure/PhotoshopCanvasGeometry.ts'],
    bundle: true,
    write: false,
    format: 'cjs',
    platform: 'neutral',
    target: 'es2022',
    external: ['photoshop'],
  });
  code = output.outputFiles[0]!.text;
});

/** Substitute native document primitives while retaining production identity and geometry policy. */
function fixture() {
  const resize = jest
    .fn<(width: number, height: number, anchor: string) => Promise<void>>()
    .mockImplementation(async (width, height) => {
      target.width = width;
      target.height = height;
    });
  const target = {
    id: 1,
    width: 512,
    height: 384,
    mode: 'RGB',
    bitsPerChannel: 8,
    colorProfileName: 'sRGB',
    pixelAspectRatio: 1,
    artboards: [],
    quickMaskMode: false,
    selection: { bounds: { left: 9.2, top: 19.8, right: 29.4, bottom: 39.1 } },
    resizeCanvas: resize,
  };
  const other = { ...target, id: 2 };
  const app = { documents: [target, other], activeDocument: target };
  const suspend = jest.fn<() => Promise<number>>().mockResolvedValue(1);
  const resume = jest
    .fn<(id: number, commit: boolean) => Promise<void>>()
    .mockResolvedValue(undefined);
  const native = {
    app,
    constants: { AnchorPosition: { MIDDLECENTER: 'center' } },
    core: {
      executeAsModal: async (run: (scope: typeof execution) => Promise<void>) => run(execution),
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
      if (name !== 'photoshop') throw new Error('Unexpected dependency');
      return native;
    },
  });
  const ctor = module.exports.PhotoshopCanvasGeometry;
  if (typeof ctor !== 'function') throw new Error('Missing native geometry adapter');
  // The compiled UXP constructor deliberately has a separate module identity.
  const Constructor = ctor as new () => CanvasGeometryHost;
  const host = new Constructor();
  return { host, app, target, other, resize, suspend, resume, execution };
}

test('reads document and selection geometry without capturing or editing any pixels', async () => {
  const f = fixture();
  expect(await f.host.execute({ type: 'context' })).toMatchObject({
    kind: 'context',
    supported: true,
    resize: true,
    document: { id: '1', width: 512, height: 384 },
    selection: { left: 9, top: 19, width: 21, height: 21 },
  });
  expect(f.resize).not.toHaveBeenCalled();
  expect(f.suspend).not.toHaveBeenCalled();
});

test('resizes canvas bounds once without resampling or switching the active document', async () => {
  const f = fixture();
  const context = await f.host.execute({ type: 'context' });
  if (context.kind !== 'context' || !context.document) throw new Error('Missing context');
  f.app.activeDocument = f.other;
  expect(
    await f.host.execute({
      type: 'resize',
      document: context.document,
      size: { width: 1024, height: 768 },
    }),
  ).toMatchObject({ kind: 'resized', document: { id: '1', width: 1024, height: 768 } });
  expect(f.resize).toHaveBeenCalledWith(1024, 768, 'center');
  expect(f.suspend).toHaveBeenCalledWith({ documentID: 1, name: 'Resize canvas' });
  expect(f.resume).toHaveBeenCalledWith(1, true);
  expect(f.app.activeDocument).toBe(f.other);
});

test('refuses stale captures and invalid pixel dimensions before mutating a document', async () => {
  const f = fixture();
  const context = await f.host.execute({ type: 'context' });
  if (context.kind !== 'context' || !context.document) throw new Error('Missing context');
  const request = {
    type: 'resize',
    document: context.document,
    size: { width: 1024, height: 768 },
  } as const;
  f.target.width = 513;
  expect(await f.host.execute(request)).toMatchObject({ kind: 'failure', status: 'stale' });
  f.target.width = 512;
  expect(await f.host.execute({ ...request, size: { width: 0, height: 768 } })).toMatchObject({
    kind: 'failure',
  });
  expect(f.resize).not.toHaveBeenCalled();
});

test('rolls back native failure and cancellation without committing a second undo state', async () => {
  const f = fixture();
  const context = await f.host.execute({ type: 'context' });
  if (context.kind !== 'context' || !context.document) throw new Error('Missing context');
  const request = {
    type: 'resize',
    document: context.document,
    size: { width: 1024, height: 768 },
  } as const;
  const cancelled = new CanvasCancellation();
  cancelled.cancel();
  expect(await f.host.execute(request, cancelled)).toMatchObject({
    kind: 'failure',
    status: 'cancelled',
  });
  expect(f.resize).not.toHaveBeenCalled();
  f.resize.mockRejectedValue(new Error('Native resize failed'));
  await expect(f.host.execute(request)).rejects.toThrow('Native resize failed');
  expect(f.resume).toHaveBeenCalledWith(1, false);
  expect(f.resume).not.toHaveBeenCalledWith(1, true);
});

test('does not report success when the native resize silently leaves the frame unchanged', async () => {
  const f = fixture();
  const context = await f.host.execute({ type: 'context' });
  if (context.kind !== 'context' || !context.document) throw new Error('Missing context');
  f.resize.mockImplementation(async () => undefined);
  await expect(
    f.host.execute({
      type: 'resize',
      document: context.document,
      size: { width: 1024, height: 768 },
    }),
  ).rejects.toThrow('did not apply');
  expect(f.resume).toHaveBeenCalledWith(1, false);
  expect(f.resume).not.toHaveBeenCalledWith(1, true);
});
