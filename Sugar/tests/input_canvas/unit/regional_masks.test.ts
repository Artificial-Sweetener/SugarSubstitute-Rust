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
/** Prove regional mask labels, the region-count generation check and picture import. */
import { jest } from '@jest/globals';
import {
  missingRegionMasks,
  regionalPromptDemand,
} from '../../../frontend/comfyui/ui/input_canvas/domain/RegionalPromptDemand.js';
import { grayscaleCoverage } from '../../../frontend/comfyui/ui/input_canvas/domain/MaskImport.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import { decodeCanvasRaster } from '../../../frontend/comfyui/ui/core/embedded/CanvasRuntime.js';
import { planWorkflowSurfaces } from '../../../frontend/comfyui/ui/input_canvas/domain/WorkflowSurfacePlan.js';
import { WorkflowMaskImport } from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowMaskImport.js';
import {
  WorkflowSurfaceProvisioning,
  type WorkflowSurfaceState,
} from '../../../frontend/comfyui/ui/input_canvas/application/WorkflowSurfaceProvisioning.js';
import type { WorkflowMaskEndpoint } from '../../../frontend/comfyui/ui/input_canvas/domain/WorkflowSurfacePlan.js';
import type { CanvasPixelRaster } from '../../../integrations/contracts/CanvasProtocol.generated.js';

/** Describe an ordered loader with the given regions and prompt demand. */
function regional(entries: number, demand: ReturnType<typeof regionalPromptDemand> | null) {
  return {
    key: 'regions',
    label: 'Load Mask Batch',
    opacity: 0.5,
    image: 'image',
    authority: { kind: 'missing', reason: 'mask_output_is_unused' },
    regions: demand,
    channel: 'alpha',
    entries: Array.from({ length: entries }, (_, index) => ({
      entry: 'r' + String(index),
      label: 'Mask ' + String(index + 1),
      initial: null,
    })),
  } satisfies WorkflowMaskEndpoint;
}

test('region names come from the first prompt naming each separator', () => {
  const demand = regionalPromptDemand([
    { path: 'positive', separators: [null, ' sky ', ''] },
    { path: 'negative', separators: ['person', 'ignored'] },
  ]);
  expect(demand).toEqual({
    names: ['person', 'sky', null],
    required: 3,
    prompts: ['positive', 'negative'],
  });
  expect(regionalPromptDemand([])).toEqual({ names: [], required: 0, prompts: [] });
  expect(missingRegionMasks(demand, 2)).toBe(1);
  expect(missingRegionMasks(demand, 4)).toBe(0);
  expect(missingRegionMasks(null, 0)).toBe(0);
  const plan = planWorkflowSurfaces(
    {
      images: [
        { key: 'image', label: 'Image', file: { filename: 'a.png', subfolder: '', type: 'input' } },
      ],
      masks: [regional(4, demand)],
    },
    '#479ef5',
    { synthetic: 'Mask canvas' },
  );
  expect(plan.surfaces[0]?.masks.map((mask) => mask.label)).toEqual([
    'person',
    'sky',
    'Mask 3',
    'Mask 4',
  ]);
});

test('generation is refused while prompts declare more regions than the loader holds', async () => {
  let masks = [regional(1, regionalPromptDemand([{ path: 'p', separators: [null, 'b'] }]))];
  const host = {
    canvasId: 'native',
    execute: jest.fn(async () => ({ kind: 'synced' as const, surfaces: [] })),
  };
  const service = new WorkflowSurfaceProvisioning(
    {
      workflow: () => 'workflow-a',
      facts: () => ({ images: [], masks }),
      binding: () => null,
      subscribe: () => () => {},
    },
    () => host,
    () => '#479ef5',
    { read: async () => null },
    jest.fn(),
    jest.fn(),
  );
  await service.assertGenerationReady();
  await service.request();
  await expect(service.assertGenerationReady()).rejects.toThrow(
    'Regional prompt is missing an associated mask: Load Mask Batch has 1 of the 2 masks',
  );
  masks = [regional(2, regionalPromptDemand([{ path: 'p', separators: [null, 'b'] }]))];
  await expect(service.assertGenerationReady()).resolves.toBeUndefined();
});

test('imported pictures read as Qt gray levels with alpha ignored', () => {
  expect([
    ...grayscaleCoverage(
      Uint8Array.from([255, 255, 255, 0, 255, 0, 0, 128, 0, 255, 0, 255, 0, 0, 255, 9]),
    ),
  ]).toEqual([255, 87, 127, 39]);
  expect(() => grayscaleCoverage(new Uint8Array(3))).toThrow(RangeError);
});

test('imports stage every picture at the surface size before adding any region', async () => {
  const state: WorkflowSurfaceState = {
    canvasId: 'native',
    workflow: 'workflow-a',
    rejected: [],
    surfaces: [
      {
        key: 'image',
        label: 'Image',
        image: 'image',
        authority: null,
        base: { kind: 'blank', width: 8, height: 4 },
        document: { id: 'doc', geometryRevision: 'g', width: 8, height: 4 },
        masks: [
          {
            key: 'k',
            endpoint: 'regions',
            entry: 'r0',
            label: 'Mask 1',
            color: '#479ef5',
            opacity: 0.5,
            initial: null,
            channel: 'alpha',
            layerId: 'layer',
          },
        ],
      },
    ],
  };
  const raster = jest.fn(
    async (_file: Blob, width: number, height: number): Promise<CanvasPixelRaster> => ({
      ...encodeCanvasRaster(width, height, 'coverage8', new Uint8Array(width * height).fill(200)),
    }),
  );
  let staged = 0;
  const stage = jest.fn(
    async (_pixels: CanvasPixelRaster) => `sugarcubes/sources/mask-${String(++staged)}.png`,
  );
  const imports = new WorkflowMaskImport(
    { state, subscribe: () => () => {} },
    { raster },
    { stage },
  );
  const append = jest.fn();
  expect(imports.available('regions')).toBe(true);
  expect(imports.available('other')).toBe(false);
  await imports.import('regions', [new Blob(['a']), new Blob(['b'])], append);
  expect(raster.mock.calls.map((call) => [call[1], call[2]])).toEqual([
    [8, 4],
    [8, 4],
  ]);
  const pixels = stage.mock.calls[0]?.[0];
  expect(pixels?.format).toBe('rgba8');
  expect(pixels ? [...decodeCanvasRaster(pixels).slice(0, 4)] : []).toEqual([200, 200, 200, 55]);
  expect(append).toHaveBeenCalledWith([
    'sugarcubes/sources/mask-1.png',
    'sugarcubes/sources/mask-2.png',
  ]);
  raster.mockRejectedValueOnce(new Error('Undecodable.'));
  await expect(imports.import('regions', [new Blob(['c'])], append)).rejects.toThrow(
    'Undecodable.',
  );
  expect(append).toHaveBeenCalledTimes(1);
  await expect(imports.import('other', [new Blob(['c'])], append)).rejects.toThrow(
    'no input canvas',
  );
});
