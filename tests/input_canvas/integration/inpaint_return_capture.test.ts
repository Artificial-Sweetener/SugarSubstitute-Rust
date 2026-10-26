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
/** Prove queued masks, image associations and document provenance survive subsequent canvas changes. */
import {
  sourceMaskFixture,
  sourceCoverage,
  sourceDocument,
} from '../support/source_mask_fixture.js';
import { compiledInputReturnAssociations } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputReturnGraph.js';
import {
  decodeCanvasRaster,
  encodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';
import { prepareCanvasImagePatch } from '../../../integrations/shared/CanvasImageRegions.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import { withInputBinding } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/InputBindingMetadata.js';

/** Exercise real capture and canonical graph attribution with only external canvas reads replaced. */
function fixture() {
  const f = sourceMaskFixture();
  f.loaders.define({
    LoadImage: { output: ['IMAGE', 'MASK'] },
    LoadImageMask: { output: ['MASK'] },
    Inpaint: { input: { required: { pixels: ['IMAGE'], mask: ['MASK'] } }, output: ['LATENT'] },
    VAEDecode: { input: { required: { samples: ['LATENT'] } }, output: ['IMAGE'] },
    SaveImage: { input: { required: { images: ['IMAGE'] } }, output: [] },
    ImageCompositeMasked: {
      input: { required: { destination: ['IMAGE'], source: ['IMAGE'], mask: ['MASK'] } },
      output: ['IMAGE'],
    },
  });
  const output = {
    '1': { class_type: 'LoadImage', inputs: { image: '' } },
    '2': { class_type: 'LoadImageMask', inputs: { image: '', channel: 'alpha' } },
    '3': { class_type: 'Inpaint', inputs: { pixels: ['1', 0], mask: ['2', 0] } },
    '4': { class_type: 'VAEDecode', inputs: { samples: ['3', 0] } },
    '5': { class_type: 'SaveImage', inputs: { images: ['4', 0] } },
    '6': { class_type: 'SaveImage', inputs: { images: ['1', 0] } },
  };
  const payload = {
    workflow: {
      nodes: [
        { id: 1, type: 'LoadImage', properties: {}, sugarcubes_widget_values: { image: '' } },
        {
          id: 2,
          type: 'LoadImageMask',
          properties: {},
          sugarcubes_widget_values: { image: '', channel: 'alpha' },
        },
      ],
    },
    output,
  };
  return { ...f, payload };
}

test('carries a non-aligned real VAE input frame through preparation, attribution and positioned masked delivery', async () => {
  const f = fixture();
  const doc = { ...sourceDocument, width: 816, height: 684 };
  const mask = new Uint8Array(doc.width * doc.height);
  mask[2 * doc.width + 5] = 127;
  mask[3 * doc.width + 6] = 255;
  mask[0] = 255;
  const coverage = encodeCanvasRaster(doc.width, doc.height, 'coverage8', mask);
  const pixels = new Uint8Array(doc.width * doc.height * 4).fill(255);
  const rgba = encodeCanvasRaster(doc.width, doc.height, 'rgba8', pixels);
  f.execute.mockImplementation(async (query) =>
    query.type === 'context'
      ? {
          kind: 'context',
          supported: true,
          document: doc,
          revision: '1',
          targets: ['composition', 'selection'],
          layers: [],
          currentLayerId: null,
        }
      : {
          kind: 'captured',
          snapshots: query.targets.map((target) => ({
            target,
            document: doc,
            revision: '1',
            pixels: target.kind === 'selection' ? coverage : rgba,
          })),
        },
  );
  f.loaders.define({
    InpaintModelConditioning: {
      input: { required: { pixels: ['IMAGE'], mask: ['MASK'], vae: ['VAE'] } },
      output: ['CONDITIONING', 'CONDITIONING', 'LATENT'],
    },
    'SimpleSyrup.KSamplerExtras': {
      input: { required: { latent_image: ['LATENT'] } },
      output: ['LATENT'],
    },
  });
  const output = {
    '1': f.payload.output['1'],
    '2': f.payload.output['2'],
    '3': {
      class_type: 'InpaintModelConditioning',
      inputs: { pixels: ['1', 0], mask: ['2', 0], vae: ['vae', 0] },
    },
    sample: { class_type: 'SimpleSyrup.KSamplerExtras', inputs: { latent_image: ['3', 2] } },
    decode: { class_type: 'VAEDecode', inputs: { samples: ['sample', 0], vae: ['vae', 0] } },
    save: { class_type: 'SaveImage', inputs: { images: ['decode', 0] } },
  };
  const prepared = await f.preparation.prepare({ workflow: f.payload.workflow, output });
  if (!isRecord(prepared.payload)) throw new Error('Missing prepared prompt.');
  const returned = prepared
    .canvas!.resolve(prepared.payload.output)
    .find((value) => value.executionPath === 'save');
  if (returned?.kind !== 'region') throw new Error('Missing VAE return region.');
  const region = returned.regions[0]!;
  expect(region.outputFrame).toBe('vae-center-crop');
  expect(region.mapping.documentBounds).toEqual({ left: 0, top: 0, width: 816, height: 684 });
  expect(region.mask).toMatchObject({ width: 816, height: 684 });
  const generated = encodeCanvasRaster(816, 680, 'rgba8', new Uint8Array(816 * 680 * 4).fill(255));
  const patch = prepareCanvasImagePatch(generated, region);
  expect(patch?.position).toEqual({ left: 5, top: 2 });
  expect(patch?.pixels).toMatchObject({ width: 2, height: 2 });
  expect([...decodeCanvasRaster(patch!.pixels)]).toEqual([
    255, 255, 255, 127, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255,
  ]);
  f.previews.dispose();
});

test('freezes actual interpreted selection and original document before the queued run, resolving through latent nodes', async () => {
  const f = fixture();
  const prepared = await f.preparation.prepare(f.payload);
  if (!isRecord(prepared.payload) || !isRecord(prepared.payload.output))
    throw new Error('Missing prepared prompt');
  const reads = f.execute.mock.calls.length;
  f.payload.output['2'].inputs.channel = 'red';
  f.execute.mockResolvedValue({ kind: 'failure', status: 'stale', message: 'Selection changed' });
  f.loaders.define({ LoadImageMask: { output: [] } });
  const plans = prepared.canvas!.resolve(prepared.payload.output);
  const returned = plans.find((plan) => plan.executionPath === '5');
  expect(returned).toMatchObject({ kind: 'region', canvasId: 'editor', document: sourceDocument });
  if (returned?.kind !== 'region') throw new Error('Missing inpaint region');
  expect([...decodeCanvasRaster(returned.regions[0]!.mask)]).toEqual([
    ...decodeCanvasRaster(sourceCoverage),
  ]);
  expect(returned.regions[0]?.coverage).toBe('mask');
  expect(returned.regions[0]?.mapping.documentBounds).toEqual({
    left: 0,
    top: 0,
    width: 3,
    height: 2,
  });
  expect(plans.some((plan) => plan.executionPath === '6')).toBe(false);
  expect(f.execute.mock.calls.length).toBe(reads);
  expect(Object.isFrozen(returned.regions[0]?.mask)).toBe(true);
  f.previews.dispose();
});

test('resolves compiler-generated node paths using exact staged input evidence', async () => {
  const f = fixture();
  const prepared = await f.preparation.prepare(f.payload);
  if (!isRecord(prepared.payload) || !isRecord(prepared.payload.output))
    throw new Error('Missing prepared prompt');
  const prompt = prepared.payload.output;
  const aliased = {
    ...prompt,
    'cube:1': prompt['1'],
    'cube:2': prompt['2'],
    '3': { class_type: 'Inpaint', inputs: { pixels: ['cube:1', 0], mask: ['cube:2', 0] } },
  };
  delete (aliased as Record<string, unknown>)['1'];
  delete (aliased as Record<string, unknown>)['2'];
  expect(prepared.canvas!.resolve(aliased).find((plan) => plan.executionPath === '5')?.kind).toBe(
    'region',
  );
  f.previews.dispose();
});

test('freezes native file-mask interpretation at full resolution with the queued channel', async () => {
  const f = fixture();
  f.payload.workflow.nodes[1]!.properties = withInputBinding({}, 'mask', {
    version: 2,
    field: 'image',
    files: ['mask.png'],
    selected: 'file',
    entries: [{ id: 'file', label: 'Mask', source: { kind: 'file', file: 'mask.png' } }],
  });
  f.payload.workflow.nodes[1]!.sugarcubes_widget_values.image = 'mask.png';
  f.payload.output['2'].inputs.image = 'mask.png';
  f.payload.output['2'].inputs.channel = 'green';
  const prepared = await f.preparation.prepare(f.payload);
  expect(f.files.load).toHaveBeenCalledWith(
    'mask.png',
    expect.objectContaining({ kind: 'mask', inputs: { channel: 'green' } }),
    0,
  );
  if (!isRecord(prepared.payload)) throw new Error('Missing prompt');
  expect(
    prepared.canvas!.resolve(prepared.payload.output).find((plan) => plan.executionPath === '5')
      ?.kind,
  ).toBe('region');
  f.previews.dispose();
});

test('refuses an unverified connected mask channel without changing the queue graph or decoding a guessed channel', async () => {
  const f = fixture();
  const output = {
    ...f.payload.output,
    '2': { class_type: 'LoadImageMask', inputs: { image: '', channel: ['channel-node', 0] } },
  };
  const prepared = await f.preparation.prepare({ ...f.payload, output });
  if (!isRecord(prepared.payload)) throw new Error('Missing prompt');
  expect(f.files.load).not.toHaveBeenCalled();
  expect(
    prepared.canvas!.resolve(prepared.payload.output).find((plan) => plan.executionPath === '5'),
  ).toMatchObject({ kind: 'unavailable', message: expect.stringContaining('graph-connected') });
  f.previews.dispose();
});

test.each([false, true])(
  "retains an image loader's connected native MASK output without treating its IMAGE branch as inpaint (direct=%s)",
  async (direct) => {
    const f = fixture();
    const workflow = {
      nodes: [
        {
          id: 1,
          type: 'LoadImage',
          properties: {},
          sugarcubes_widget_values: { image: '' },
          outputs: [
            { name: 'IMAGE', type: 'IMAGE', links: [1] },
            { name: 'MASK', type: 'MASK', links: [2] },
          ],
        },
      ],
    };
    const output = {
      '1': { class_type: 'LoadImage', inputs: { image: '' } },
      '3': { class_type: 'Inpaint', inputs: { pixels: ['1', 0], mask: ['1', 1] } },
      '4': f.payload.output['4'],
      '5': f.payload.output['5'],
      '6': f.payload.output['6'],
    };
    const prepared = await f.preparation.prepare(direct ? { workflow } : { workflow, output });
    expect(prepared.canvas).toBeDefined();
    if (
      !isRecord(prepared.payload) ||
      !isRecord(prepared.payload.workflow) ||
      !Array.isArray(prepared.payload.workflow.nodes)
    )
      throw new Error('Missing prepared workflow');
    const node = prepared.payload.workflow.nodes[0];
    if (!isRecord(node) || !isRecord(node.sugarcubes_widget_values))
      throw new Error('Missing captured image loader');
    const canonical = {
      ...output,
      '1': { class_type: 'LoadImage', inputs: { image: node.sugarcubes_widget_values.image } },
    };
    const plans = prepared.canvas!.resolve(canonical);
    const returned = plans.find((plan) => plan.executionPath === '5');
    expect(returned).toMatchObject({
      kind: 'region',
      canvasId: 'editor',
      document: sourceDocument,
    });
    if (returned?.kind !== 'region') throw new Error('Missing derived return mask');
    expect([...decodeCanvasRaster(returned.regions[0]!.mask)]).toEqual([255, 128, 0, 128, 0, 255]);
    expect(plans.some((plan) => plan.executionPath === '6')).toBe(false);
    f.previews.dispose();
  },
);

test('does not prepare a return mask for an image loader whose MASK socket is unused', async () => {
  const f = fixture();
  const prepared = await f.preparation.prepare({
    workflow: { nodes: [f.payload.workflow.nodes[0]] },
    output: {
      '1': f.payload.output['1'],
      '6': f.payload.output['6'],
    },
  });
  expect(prepared.canvas).toBeUndefined();
  expect(f.files.load).not.toHaveBeenCalled();
  f.previews.dispose();
});

test('refuses ambiguous image sources and distinguishes an already composited output', () => {
  const f = fixture();
  const captured = [
    {
      key: 'image',
      nodePath: '1',
      classType: 'LoadImage',
      field: 'image',
      kind: 'image' as const,
      files: ['image.png'],
    },
    {
      key: 'mask',
      nodePath: '2',
      classType: 'LoadImageMask',
      field: 'image',
      kind: 'mask' as const,
      files: ['mask.png'],
    },
  ];
  const prompt = {
    ...f.payload.output,
    '1': { class_type: 'LoadImage', inputs: { image: 'image.png' } },
    '2': { class_type: 'LoadImageMask', inputs: { image: 'mask.png', channel: 'alpha' } },
    '7': {
      class_type: 'ImageCompositeMasked',
      inputs: { source: ['4', 0], destination: ['1', 0], mask: ['2', 0] },
    },
    '8': { class_type: 'SaveImage', inputs: { images: ['7', 0] } },
  };
  expect(
    compiledInputReturnAssociations(prompt, captured, f.loaders.assetPorts()).find(
      (plan) => plan.output === '8',
    ),
  ).toMatchObject({ kind: 'associated', composited: true });
  const competing = {
    ...prompt,
    '9': { class_type: 'LoadImage', inputs: { image: 'other.png' } },
    '10': { class_type: 'Inpaint', inputs: { pixels: ['9', 0], mask: ['2', 0] } },
  };
  const both = [
    ...captured,
    { ...captured[0]!, key: 'other', nodePath: '9', files: ['other.png'] },
  ];
  expect(
    compiledInputReturnAssociations(competing, both, f.loaders.assetPorts()).find(
      (plan) => plan.output === '8',
    )?.kind,
  ).toBe('unavailable');
  f.previews.dispose();
});
