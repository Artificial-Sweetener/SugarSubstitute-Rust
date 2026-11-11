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
/** Prove queue-time capture of a provisioned mask layer beside a file-based image. */
import { jest } from '@jest/globals';
import { CanvasSourceCapture } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourceCapture.js';
import { InputSourcePreparation } from '../../../frontend/comfyui/ui/input_canvas/application/InputSourcePreparation.js';
import { MaskInputPreparation } from '../../../frontend/comfyui/ui/input_canvas/application/MaskInputPreparation.js';
import { editMaskInput } from '../../../frontend/comfyui/ui/input_canvas/domain/MaskInputEditing.js';
import type { InputBinding } from '../../../frontend/comfyui/ui/input_canvas/domain/InputBinding.js';
import { ComfyInputLoaders } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputLoaders.js';
import { ComfyInputPreparation } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputPreparation.js';
import { ComfyInputReturnPreparation } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputReturnPreparation.js';
import { ComfyInputSourceDefaults } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputSourceDefaults.js';
import { ComfyMaskInputPreparation } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyMaskInputPreparation.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import type { CanvasSourceHost } from '../../../integrations/contracts/CanvasSources.js';
import type {
  CanvasDocument,
  CanvasPixelRaster,
  CanvasSourceQuery,
  CanvasSourceResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

const inputDocument: CanvasDocument = {
  id: 'doc-input',
  geometryRevision: 'g1',
  width: 2,
  height: 1,
};
/** A mask layer in the coverage image encoding: rgb = value, alpha = 255 - value. */
const maskLayer = encodeCanvasRaster(
  2,
  1,
  'rgba8',
  Uint8Array.from([255, 255, 255, 0, 0, 0, 0, 255]),
);

/** Queue preparation over a canvas whose active route shows Output, so no document is active. */
function fixture(documents: (id: string) => CanvasDocument | null = () => inputDocument) {
  const execute = jest.fn(
    async (query: CanvasSourceQuery): Promise<CanvasSourceResult> =>
      query.type === 'context'
        ? {
            kind: 'context',
            observesChanges: true,
            supported: true,
            document: null,
            revision: '7',
            targets: ['selection', 'current-layer', 'layer', 'composition'],
            layers: [],
            currentLayerId: null,
          }
        : {
            kind: 'captured',
            snapshots: query.targets.map((target) => ({
              target,
              document: query.document,
              revision: '7',
              pixels: maskLayer,
            })),
          },
  );
  const host: CanvasSourceHost = { canvasId: 'native', execute, changes: null };
  const loaders = new ComfyInputLoaders();
  loaders.define({
    LoadImage: { output: ['IMAGE', 'MASK'] },
    LoadImageMask: { output: ['MASK'] },
  });
  const stage = jest.fn(async (_pixels: CanvasPixelRaster) => 'sugarcubes/sources/mask-1.png');
  const capture = new CanvasSourceCapture(
    (id) => (id === 'native' ? host : null),
    (canvasId, documentId) => (canvasId === 'native' ? documents(documentId) : null),
  );
  const unused = jest.fn<never>();
  const preparation = new ComfyInputPreparation(
    loaders,
    new InputSourcePreparation(capture, { stage }),
    new ComfyInputSourceDefaults(
      () => host,
      { read: async () => null },
      () => true,
    ),
    new ComfyInputReturnPreparation({ load: unused }, async () => null),
    new ComfyMaskInputPreparation(
      { load: unused },
      { stage },
      new MaskInputPreparation({
        prepare: async (source, settings) => editMaskInput(source, settings),
      }),
      () => new Map(),
    ),
    { prepare: async (prompt) => prompt },
  );
  return { execute, stage, preparation };
}

/** A workflow whose image loader keeps its file and whose mask loader is bound to its layer. */
function payload() {
  const binding: InputBinding = {
    version: 2,
    field: 'image',
    files: [],
    selected: 'mask',
    entries: [
      {
        id: 'mask',
        label: 'Load Mask',
        source: {
          kind: 'canvas',
          canvasId: 'native',
          documentId: 'doc-input',
          target: { kind: 'layer', layerId: 'layer-1' },
        },
      },
    ],
  };
  return {
    workflow: {
      nodes: [
        {
          id: 1,
          type: 'LoadImage',
          properties: {},
          sugarcubes_widget_values: { image: 'photo.png' },
          widgets_values: ['photo.png', 'image'],
        },
        {
          id: 2,
          type: 'LoadImageMask',
          properties: { sugarcubes_canvas_masks: binding },
          sugarcubes_widget_values: { image: '', channel: 'alpha' },
          widgets_values: ['', 'alpha'],
        },
      ],
      links: [
        [1, 1, 0, 3, 0, 'IMAGE'],
        [2, 2, 0, 3, 1, 'MASK'],
      ],
    },
    output: {
      '1': { class_type: 'LoadImage', inputs: { image: 'photo.png' } },
      '2': { class_type: 'LoadImageMask', inputs: { image: '', channel: 'alpha' } },
      '3': { class_type: 'Inpaint', inputs: { pixels: ['1', 0], mask: ['2', 0] } },
    },
  };
}

test('the provisioned mask layer is captured from its inactive document and the image keeps its file', async () => {
  const f = fixture();
  const prepared = await f.preparation.prepare(payload());
  expect(prepared.payload).toMatchObject({
    output: {
      '1': { inputs: { image: 'photo.png' } },
      '2': { inputs: { image: 'sugarcubes/sources/mask-1.png', channel: 'alpha' } },
    },
  });
  expect(f.execute).toHaveBeenLastCalledWith({
    type: 'capture',
    document: inputDocument,
    targets: [{ kind: 'layer', layerId: 'layer-1' }],
    maximumSize: 0,
  });
  expect(f.stage).toHaveBeenCalledTimes(1);
});

test('a retired surface, a refused capture or a stale document fails generation closed', async () => {
  const retired = fixture(() => null);
  await expect(retired.preparation.prepare(payload())).rejects.toThrow('Open a document');
  for (const failure of ['stale', 'unavailable'] as const) {
    const refused = fixture();
    refused.execute.mockImplementation(async (query) =>
      query.type === 'context'
        ? {
            kind: 'context',
            observesChanges: true,
            supported: true,
            document: null,
            revision: '7',
            targets: ['layer'],
            layers: [],
            currentLayerId: null,
          }
        : { kind: 'failure', status: failure, message: 'The input document changed.' },
    );
    await expect(refused.preparation.prepare(payload())).rejects.toThrow('Source unavailable');
    expect(refused.stage).not.toHaveBeenCalled();
  }
  const resized = fixture(() => ({ ...inputDocument, width: 4 }));
  await expect(resized.preparation.prepare(payload())).rejects.toThrow('dimensions changed');
});
