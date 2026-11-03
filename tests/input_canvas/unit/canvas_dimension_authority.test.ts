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
/** Prove how an unpaired mask finds the authored size of its mask-only surface. */
import {
  inferDimensionFieldPair,
  resolveDimensionAuthority,
  type SpatialEdge,
  type SpatialNode,
} from '../../../frontend/comfyui/ui/input_canvas/domain/CanvasDimensionAuthority.js';

const size = { width: 'width', height: 'height' };

/** Build a mask → SetLatentNoiseMask → KSampler graph whose latent comes from `latent`. */
function graph(extra: { nodes?: SpatialNode[]; edges?: SpatialEdge[] } = {}) {
  const nodes: SpatialNode[] = [
    { key: 'mask', outputs: ['MASK'], dimensions: [] },
    { key: 'latent', outputs: ['LATENT'], dimensions: [size] },
    { key: 'noise', outputs: ['LATENT'], dimensions: [] },
    { key: 'sampler', outputs: ['LATENT'], dimensions: [] },
    ...(extra.nodes ?? []),
  ];
  const edges: SpatialEdge[] = [
    { source: 'mask', output: 0, type: 'MASK', target: 'noise', inputType: 'MASK' },
    { source: 'latent', output: 0, type: 'LATENT', target: 'noise', inputType: 'LATENT' },
    { source: 'noise', output: 0, type: 'LATENT', target: 'sampler', inputType: 'LATENT' },
    ...(extra.edges ?? []),
  ];
  return { nodes, edges };
}

/** Read widget values from a plain table. */
function values(table: Record<string, Record<string, unknown>>) {
  return (node: string, field: string): unknown => table[node]?.[field];
}

test('a latent root meeting the mask downstream sizes it', () => {
  const authority = resolveDimensionAuthority(
    graph(),
    'mask',
    values({ latent: { width: 1024, height: 768 } }),
  );
  expect(authority).toEqual({
    kind: 'resolved',
    key: JSON.stringify([['latent', 0, 'width', 'height']]),
    width: 1024,
    height: 768,
    nodes: [{ node: 'latent', output: 0, width: 'width', height: 'height' }],
  });
});

test('latent roots win over image roots and disagreeing candidates are ambiguous', () => {
  const image = {
    nodes: [{ key: 'image', outputs: ['IMAGE'], dimensions: [size] }],
    edges: [{ source: 'image', output: 0, type: 'IMAGE', target: 'sampler', inputType: 'IMAGE' }],
  };
  expect(
    resolveDimensionAuthority(
      graph(image),
      'mask',
      values({ latent: { width: 512, height: 512 }, image: { width: 64, height: 64 } }),
    ),
  ).toMatchObject({ kind: 'resolved', width: 512, height: 512 });
  const second = {
    nodes: [{ key: 'latent-b', outputs: ['LATENT'], dimensions: [size] }],
    edges: [
      { source: 'latent-b', output: 0, type: 'LATENT', target: 'sampler', inputType: 'LATENT' },
    ],
  };
  expect(
    resolveDimensionAuthority(
      graph(second),
      'mask',
      values({ latent: { width: 512, height: 512 }, 'latent-b': { width: 768, height: 512 } }),
    ),
  ).toEqual({
    kind: 'ambiguous',
    reason: 'relevant_spatial_roots_disagree',
    candidates: ['latent', 'latent-b'],
  });
  const agreeing = resolveDimensionAuthority(
    graph(second),
    'mask',
    values({ latent: { width: 512, height: 512 }, 'latent-b': { width: 512, height: 512 } }),
  );
  expect(agreeing.kind === 'resolved' ? agreeing.nodes.map((node) => node.node) : []).toEqual([
    'latent',
    'latent-b',
  ]);
});

test('unused masks, linked or invalid sizes and spatially fed nodes give no authority', () => {
  expect(
    resolveDimensionAuthority({ nodes: graph().nodes, edges: [] }, 'mask', values({})),
  ).toEqual({ kind: 'missing', reason: 'mask_output_is_unused' });
  for (const invalid of [{ width: 0, height: 64 }, { width: true, height: 64 }, { width: 1.5 }])
    expect(resolveDimensionAuthority(graph(), 'mask', values({ latent: invalid }))).toEqual({
      kind: 'missing',
      reason: 'no_relevant_spatial_root_with_dimensions',
    });
  const fed = graph({
    nodes: [{ key: 'upscale', outputs: ['IMAGE'], dimensions: [size] }],
    edges: [
      { source: 'other', output: 0, type: 'IMAGE', target: 'upscale', inputType: 'IMAGE' },
      { source: 'upscale', output: 0, type: 'IMAGE', target: 'sampler', inputType: 'IMAGE' },
    ],
  });
  expect(
    resolveDimensionAuthority(
      { ...fed, nodes: fed.nodes.filter((node) => node.key !== 'latent') },
      'mask',
      values({ upscale: { width: 64, height: 64 } }),
    ),
  ).toEqual({ kind: 'missing', reason: 'no_relevant_spatial_root_with_dimensions' });
});

test('exactly one width and height pair is inferred from field names', () => {
  expect(inferDimensionFieldPair(['seed', 'width', 'height'])).toEqual(size);
  expect(inferDimensionFieldPair(['target_width', 'target_height'])).toEqual({
    width: 'target_width',
    height: 'target_height',
  });
  expect(inferDimensionFieldPair(['width', 'height', 'crop_width', 'crop_height'])).toBeNull();
  expect(inferDimensionFieldPair(['width', 'crop_height'])).toBeNull();
});
