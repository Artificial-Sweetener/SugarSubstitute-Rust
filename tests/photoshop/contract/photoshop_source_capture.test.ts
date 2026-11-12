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
/** Exercise native source translation, current-layer identity and temporary composition visibility. */
import { jest } from '@jest/globals';
import { build } from 'esbuild';
import { runInNewContext } from 'node:vm';
import type { CanvasSourceHost } from '../../../integrations/contracts/CanvasSources.js';
import { decodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import { BUILD_FIXTURE_SETUP_TIMEOUT_MS } from '../../frontend_core/support/BuildFixtureBudget.js';

let code = '';
beforeAll(async () => {
  const bundle = await build({
    entryPoints: ['integrations/photoshop/src/infrastructure/PhotoshopCanvasSources.ts'],
    bundle: true,
    write: false,
    format: 'cjs',
    platform: 'neutral',
    target: 'es2022',
    external: ['photoshop'],
  });
  const output = bundle.outputFiles[0];
  if (!output) throw new Error('Missing native source bundle.');
  code = output.text;
}, BUILD_FIXTURE_SETUP_TIMEOUT_MS);

/** Supply only Photoshop's native boundary while executing the complete authored source adapter. */
function fixture() {
  const layers = [
    { id: 3, name: 'Top', visible: true, layers: null },
    { id: 2, name: 'Chosen', visible: true, layers: null },
    { id: 1, name: 'Bottom', visible: true, layers: null },
  ];
  const doc = {
    id: 10,
    width: 4,
    height: 2,
    mode: 'RGB',
    bitsPerChannel: 'bitDepth8',
    colorProfileName: 'sRGB',
    pixelAspectRatio: 1,
    quickMaskMode: false,
    artboards: [] as unknown[],
    activeHistoryState: { id: 1 },
    layers,
    activeLayers: [layers[0], layers[1]],
    selection: { bounds: null },
  };
  const app = { activeDocument: doc, documents: [doc] };
  const report = jest.fn(),
    resume = jest.fn(async (_id: number, _commit: boolean) => {}),
    dispose = jest.fn();
  let fail = false;
  const reads: { layerID: number | undefined; visible: readonly number[] }[] = [];
  const getPixels = jest.fn(async (options: { layerID?: number }) => {
    reads.push({
      layerID: options.layerID,
      visible: layers.filter((layer) => layer.visible).map((layer) => layer.id),
    });
    if (fail) throw new Error('Native raster read failed.');
    return {
      level: 0,
      sourceBounds: { left: 1, top: 0, right: 3, bottom: 1 },
      imageData: {
        width: 2,
        height: 1,
        components: 4,
        componentSize: 8,
        getData: async () => Uint8Array.from([201, 102, 53, 0, 11, 22, 33, 127]),
        dispose,
      },
    };
  });
  const module: { exports: Record<string, unknown> } = { exports: {} };
  runInNewContext(code, {
    module,
    Error,
    setInterval: () => 1,
    clearInterval: () => {},
    require: () => ({
      app,
      action: { batchPlay: async () => [{ layerID: 2 }] },
      imaging: { getPixels },
      core: {
        executeAsModal: async (action: (execution: unknown) => Promise<void>) => {
          await action({ hostControl: { suspendHistory: async () => 17, resumeHistory: resume } });
        },
      },
    }),
  });
  // The compiled native adapter has an isolated constructor identity across the VM boundary.
  const Constructor = module.exports.PhotoshopCanvasSources as new (
    report: (error: unknown) => void,
  ) => CanvasSourceHost & { dispose(): void };
  return {
    host: new Constructor(report),
    doc,
    layers,
    reads,
    report,
    resume,
    dispose,
    fail: () => {
      fail = true;
    },
  };
}

test('captures every source target in full-canvas coordinates and resolves the primary multiselected layer', async () => {
  const f = fixture(),
    context = await f.host.execute({ type: 'context' });
  expect(context).toMatchObject({
    kind: 'context',
    currentLayerId: '2',
    layers: [{ id: '3' }, { id: '2' }, { id: '1' }],
  });
  if (context.kind !== 'context' || !context.document) throw new Error('Missing native document.');
  const result = await f.host.execute({
    type: 'capture',
    document: context.document,
    maximumSize: 0,
    targets: [
      { kind: 'current-layer' },
      { kind: 'layer', layerId: '1' },
      { kind: 'composition' },
      { kind: 'composition-from-layer', layerId: '2' },
      { kind: 'selection' },
    ],
  });
  if (result.kind !== 'captured') throw new Error('Missing native capture.');
  expect(f.reads).toEqual([
    { layerID: 2, visible: [3, 2, 1] },
    { layerID: 1, visible: [3, 2, 1] },
    { layerID: undefined, visible: [3, 2, 1] },
    { layerID: undefined, visible: [2, 1] },
  ]);
  expect(new Set(result.snapshots.map((snapshot) => snapshot.revision)).size).toBe(1);
  const image = result.snapshots[0]?.pixels,
    selection = result.snapshots[4]?.pixels;
  if (!image || !selection) throw new Error('Missing source pixels.');
  expect([image.width, image.height, image.format]).toEqual([4, 2, 'rgba8']);
  expect([...decodeCanvasRaster(image)]).toEqual([
    0,
    0,
    0,
    0,
    201,
    102,
    53,
    0,
    11,
    22,
    33,
    127,
    ...new Array(20).fill(0),
  ]);
  expect(selection.format).toBe('coverage8');
  expect([...decodeCanvasRaster(selection)]).toEqual(new Array(8).fill(0));
  expect(f.layers.every((layer) => layer.visible)).toBe(true);
  expect(f.resume).toHaveBeenCalledWith(17, false);
  expect(f.dispose).toHaveBeenCalledTimes(4);
  expect(f.report).not.toHaveBeenCalled();
  f.host.dispose();
});

test('restores composition visibility and rolls back history even when pixel capture fails', async () => {
  const f = fixture(),
    context = await f.host.execute({ type: 'context' });
  if (context.kind !== 'context' || !context.document) throw new Error('Missing native document.');
  f.fail();
  const result = await f.host.execute({
    type: 'capture',
    document: context.document,
    maximumSize: 0,
    targets: [{ kind: 'composition-from-layer', layerId: '2' }],
  });
  expect(result).toMatchObject({ kind: 'failure', message: 'Native raster read failed.' });
  expect(f.reads[0]?.visible).toEqual([2, 1]);
  expect(f.layers.every((layer) => layer.visible)).toBe(true);
  expect(f.resume).toHaveBeenCalledWith(17, false);
  expect(f.report).toHaveBeenCalledTimes(1);
  f.host.dispose();
});

test('rejects stale geometry and explicit layers instead of reading a replacement target', async () => {
  const f = fixture(),
    context = await f.host.execute({ type: 'context' });
  if (context.kind !== 'context' || !context.document) throw new Error('Missing native document.');
  const stale = await f.host.execute({
    type: 'capture',
    document: { ...context.document, id: 'another' },
    maximumSize: 0,
    targets: [{ kind: 'composition' }],
  });
  expect(stale.kind).toBe('failure');
  expect(f.reads).toHaveLength(0);
  const missing = await f.host.execute({
    type: 'capture',
    document: context.document,
    maximumSize: 0,
    targets: [{ kind: 'layer', layerId: 'missing' }],
  });
  expect(missing).toMatchObject({
    kind: 'failure',
    message: 'The Photoshop layer source layer (missing) is unavailable.',
  });
  expect(f.reads).toHaveLength(0);
  f.host.dispose();
});
