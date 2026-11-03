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
/** Prove which surfaces and masks a workflow provisions from graph facts. */
import {
  comfyFileReference,
  planWorkflowSurfaces,
  workflowMaskKey,
  workflowSyncSurfaces,
  MAX_SURFACE_MASKS,
  type WorkflowMaskEndpoint,
} from '../../../frontend/comfyui/ui/input_canvas/domain/WorkflowSurfacePlan.js';
import type { DimensionAuthority } from '../../../frontend/comfyui/ui/input_canvas/domain/CanvasDimensionAuthority.js';

const ACCENT = '#479ef5';
const LABELS = { synthetic: 'Mask canvas' };
const photo = { filename: 'photo.png', subfolder: '', type: 'input' } as const;
const unsized: DimensionAuthority = {
  kind: 'missing',
  reason: 'no_relevant_spatial_root_with_dimensions',
};

/** Describe one scalar mask endpoint. */
function mask(
  key: string,
  image: WorkflowMaskEndpoint['image'],
  extra: Partial<WorkflowMaskEndpoint> = {},
) {
  return {
    key,
    label: key,
    opacity: 0.5,
    image,
    authority: unsized,
    regions: null,
    channel: 'red',
    entries: [{ entry: null, label: key, initial: null }],
    ...extra,
  } satisfies WorkflowMaskEndpoint;
}

/** Describe a resolved size authority shared by every mask that names it. */
function sized(node: string, width: number, height: number): DimensionAuthority {
  const nodes = [{ node, output: 0, width: 'width', height: 'height' }];
  return { kind: 'resolved', key: JSON.stringify(nodes), width, height, nodes };
}

test('paired masks join their image surface in endpoint order with rotated colours', () => {
  const plan = planWorkflowSurfaces(
    {
      images: [{ key: 'image', label: 'Load Image', file: photo }],
      masks: [mask('first', 'image'), mask('second', 'image', { opacity: 0.25 })],
    },
    ACCENT,
    LABELS,
  );
  expect(plan.rejected).toEqual([]);
  expect(plan.surfaces).toHaveLength(1);
  const [surface] = plan.surfaces;
  expect(surface).toMatchObject({
    key: 'image',
    image: 'image',
    authority: null,
    base: { kind: 'file', file: photo },
  });
  expect(surface?.masks.map((item) => [item.key, item.color, item.opacity])).toEqual([
    ['first', ACCENT, 0.5],
    ['second', expect.not.stringMatching(ACCENT), 0.25],
  ]);
  expect(workflowSyncSurfaces(plan)[0]?.masks[1]).toEqual({
    key: 'second',
    label: 'second',
    color: surface?.masks[1]?.color,
    opacity: 0.25,
    initial: null,
    channel: 'red',
  });
});

test('ambiguous pairings and unsized unpaired masks are rejected rather than guessed', () => {
  const plan = planWorkflowSurfaces(
    {
      images: [{ key: 'image', label: 'Image', file: photo }],
      masks: [
        mask('ambiguous', 'ambiguous', { authority: sized('latent', 64, 64) }),
        mask('unsized', null),
        mask('disagree', null, {
          authority: {
            kind: 'ambiguous',
            reason: 'relevant_spatial_roots_disagree',
            candidates: ['a', 'b'],
          },
        }),
      ],
    },
    ACCENT,
    LABELS,
  );
  expect(plan.surfaces.map((surface) => surface.masks.length)).toEqual([0]);
  expect(plan.rejected).toEqual([
    { endpoint: 'ambiguous', reason: 'ambiguous_image' },
    { endpoint: 'unsized', reason: 'unsized' },
    { endpoint: 'disagree', reason: 'ambiguous_size' },
  ]);
});

test('unpaired masks sized by one authority share one blank mask-only surface', () => {
  const authority = sized('5:latent', 1024, 768);
  const plan = planWorkflowSurfaces(
    {
      images: [],
      masks: [
        mask('a', null, { authority }),
        mask('b', null, { authority }),
        mask('c', null, { authority: sized('other', 512, 512) }),
      ],
    },
    ACCENT,
    LABELS,
  );
  expect(plan.surfaces.map((surface) => [surface.base, surface.masks.map((m) => m.key)])).toEqual([
    [{ kind: 'blank', width: 1024, height: 768 }, ['a', 'b']],
    [{ kind: 'blank', width: 512, height: 512 }, ['c']],
  ]);
  const [first] = plan.surfaces;
  expect(first?.key).toMatch(/^@synthetic\/[\da-f]{16}$/);
  expect(first?.authority).toEqual(authority.kind === 'resolved' ? authority.nodes : null);
  expect(first?.label).toBe('a');
  const again = planWorkflowSurfaces(
    { images: [], masks: [mask('a', null, { authority: sized('5:latent', 2048, 2048) })] },
    ACCENT,
    LABELS,
  );
  expect(again.surfaces[0]?.key).toBe(first?.key);
});

test('ordered loaders contribute one mask per region keyed by entry identity', () => {
  const plan = planWorkflowSurfaces(
    {
      images: [{ key: 'image', label: 'Image', file: photo }],
      masks: [
        mask('regions', 'image', {
          entries: [
            { entry: 'r1', label: 'Mask 1', initial: null },
            { entry: 'r2', label: 'Mask 2', initial: photo },
          ],
        }),
      ],
    },
    ACCENT,
    LABELS,
  );
  expect(plan.surfaces[0]?.masks.map((item) => [item.key, item.entry, item.initial])).toEqual([
    [workflowMaskKey('regions', 'r1'), 'r1', null],
    [workflowMaskKey('regions', 'r2'), 'r2', photo],
  ]);
  expect(workflowMaskKey('regions', null)).toBe('regions');
});

test('images without a file get no surface and mask counts beyond the contract are rejected', () => {
  const entries = Array.from({ length: MAX_SURFACE_MASKS + 1 }, (_, index) => ({
    entry: String(index),
    label: String(index),
    initial: null,
  }));
  const plan = planWorkflowSurfaces(
    {
      images: [
        { key: 'empty', label: 'Empty', file: null },
        { key: 'image', label: 'Image', file: photo },
      ],
      masks: [mask('orphan', 'empty'), mask('many', 'image', { entries })],
    },
    ACCENT,
    LABELS,
  );
  expect(plan.surfaces.map((surface) => surface.key)).toEqual(['image']);
  expect(plan.surfaces[0]?.masks).toHaveLength(MAX_SURFACE_MASKS);
  expect(plan.rejected).toEqual([{ endpoint: 'many', reason: 'limit' }]);
});

test('Comfy loader values become surface file references', () => {
  expect(comfyFileReference('photo.png')).toEqual(photo);
  expect(comfyFileReference('clipspace/sub/mask.png [output]')).toEqual({
    filename: 'mask.png',
    subfolder: 'clipspace/sub',
    type: 'output',
  });
  expect(comfyFileReference('dir\\name.webp [temp]')).toEqual({
    filename: 'name.webp',
    subfolder: 'dir',
    type: 'temp',
  });
  expect(comfyFileReference('../escape.png')).toBeNull();
  expect(comfyFileReference('folder/')).toBeNull();
});
