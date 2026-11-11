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
/** Prove paired input geometry and effective coverage survive both execution routes. */
import { sourceMaskFixture, sourceDocument, sourceRgba } from '../support/source_mask_fixture.js';
import {
  encodeCanvasRaster,
  decodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';
import { withInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/InputBindingMetadata.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

/** Replace native capture and projection transport while retaining all preparation and attribution owners. */
function fixture(crop_factor: number) {
  const f = sourceMaskFixture();
  const mask = encodeCanvasRaster(3, 2, 'coverage8', Uint8Array.from([0, 64, 0, 0, 255, 0]));
  f.execute.mockImplementation(async (query) =>
    query.type === 'context'
      ? {
          kind: 'context',
          supported: true,
          observesChanges: true,
          document: sourceDocument,
          revision: '1',
          targets: ['selection', 'composition'],
          layers: [],
          currentLayerId: null,
        }
      : {
          kind: 'captured',
          snapshots: query.targets.map((target) => ({
            target,
            document: sourceDocument,
            revision: '1',
            pixels: target.kind === 'selection' ? mask : sourceRgba,
          })),
        },
  );
  f.loaders.define({
    LoadImage: { output: ['IMAGE', 'MASK'] },
    LoadImageMask: { output: ['MASK'] },
    Inpaint: { input: { required: { image: ['IMAGE'], mask: ['MASK'] } }, output: ['IMAGE'] },
    SaveImage: { input: { required: { images: ['IMAGE'] } }, output: [] },
  });
  f.editingPairs.set(JSON.stringify(['2', 'image']), JSON.stringify(['1', 'image']));
  f.projectInputs.mockImplementation(async (prompt, assets) => {
    if (!isRecord(prompt)) throw new Error('Missing canonical prompt.');
    const asset = assets[0];
    if (!asset) throw new Error('Missing prepared mask.');
    return {
      ...prompt,
      prepared: {
        class_type: 'LoadImageMask',
        inputs: { image: asset.preparedFile, channel: 'alpha' },
      },
      '3': { class_type: 'Inpaint', inputs: { image: ['1', 0], mask: ['prepared', 0] } },
    };
  });
  const workflow = {
    nodes: [
      { id: 1, type: 'LoadImage', properties: {}, sugarcubes_widget_values: { image: '' } },
      {
        id: 2,
        type: 'LoadImageMask',
        properties: withInputBinding({}, 'mask', {
          version: 2,
          field: 'image',
          selected: 'selection',
          files: [],
          entries: [
            {
              id: 'selection',
              label: 'Selection',
              source: {
                kind: 'canvas',
                canvasId: 'editor',
                documentId: null,
                target: { kind: 'selection' },
              },
            },
          ],
          edits: { mask_dilation: 0, crop_factor, post_dilation: 0, feather: 0 },
        }),
        sugarcubes_widget_values: { image: '', channel: 'alpha' },
      },
    ],
  };
  const output = {
    '1': { class_type: 'LoadImage', inputs: { image: '' } },
    '2': { class_type: 'LoadImageMask', inputs: { image: '', channel: 'alpha' } },
    '3': { class_type: 'Inpaint', inputs: { image: ['1', 0], mask: ['2', 0] } },
    '4': { class_type: 'SaveImage', inputs: { images: ['3', 0] } },
  };
  return { ...f, workflow, output };
}

test.each([false, true])(
  'freezes the same shaped mask and image crop with correct return placement (direct=%s)',
  async (direct) => {
    const f = fixture(1),
      before = JSON.stringify(f.workflow);
    const result = await f.preparation.prepare(
      direct ? { workflow: f.workflow } : { workflow: f.workflow, output: f.output },
    );
    expect(JSON.stringify(f.workflow)).toBe(before);
    const cropped = f.stage.mock.calls.map(([pixels]) => pixels).filter((p) => p.width === 1);
    expect(cropped.map((p) => [p.width, p.height, p.format])).toEqual([
      [1, 2, 'rgba8'],
      [1, 2, 'rgba8'],
    ]);
    expect([...decodeCanvasRaster(cropped[0]!)]).toEqual([40, 50, 60, 127, 130, 140, 150, 255]);
    // Native inverse alpha encodes the unchanged soft shape, rather than filling its crop rectangle.
    expect([...decodeCanvasRaster(cropped[1]!)]).toEqual([64, 64, 64, 191, 255, 255, 255, 0]);
    expect(isRecord(result.payload)).toBe(true);
    if (
      !isRecord(result.payload) ||
      !isRecord(result.payload.workflow) ||
      !Array.isArray(result.payload.workflow.nodes)
    )
      throw new Error('Missing workflow.');
    const image = result.payload.workflow.nodes[0];
    if (!isRecord(image) || !isRecord(image.sugarcubes_widget_values))
      throw new Error('Missing source.');
    let canonical: unknown = result.payload.output;
    if (direct) {
      expect(result.preparedInputs).toHaveLength(1);
      canonical = await f.projectInputs(
        {
          ...f.output,
          '1': { class_type: 'LoadImage', inputs: { image: image.sugarcubes_widget_values.image } },
        },
        result.preparedInputs!,
      );
    } else expect(result.preparedInputs).toBeUndefined();
    f.execute.mockRejectedValue(new Error('Live canvas changed after submission.'));
    const returned = result.canvas!.resolve(canonical).find((plan) => plan.executionPath === '4');
    expect(returned).toMatchObject({
      kind: 'region',
      canvasId: 'editor',
      document: sourceDocument,
    });
    if (returned?.kind !== 'region') throw new Error('Missing return region.');
    expect(returned.regions[0]?.mapping).toEqual({
      imageSize: { width: 1, height: 2 },
      imageBounds: { left: 0, top: 0, width: 1, height: 2 },
      documentBounds: { left: 1, top: 0, width: 1, height: 2 },
    });
    expect([...decodeCanvasRaster(returned.regions[0]!.mask)]).toEqual([64, 255]);
    f.previews.dispose();
  },
);

test('default factor preserves full inputs and uses the ordinary queue graph', async () => {
  const f = fixture(0);
  const result = await f.preparation.prepare({ workflow: f.workflow, output: f.output });
  expect(f.projectInputs).not.toHaveBeenCalled();
  expect(f.stage).toHaveBeenCalledTimes(2);
  if (!isRecord(result.payload)) throw new Error('Missing prompt.');
  const returned = result
    .canvas!.resolve(result.payload.output)
    .find((plan) => plan.executionPath === '4');
  expect(returned?.kind).toBe('region');
  if (returned?.kind !== 'region') throw new Error('Missing region.');
  expect(returned.regions[0]?.mapping.documentBounds).toEqual({
    left: 0,
    top: 0,
    width: 3,
    height: 2,
  });
  expect([...decodeCanvasRaster(returned.regions[0]!.mask)]).toEqual([0, 64, 0, 0, 255, 0]);
  f.previews.dispose();
});
