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
/** Characterize Comfy media lookup for the Cube preview rail. */

import { ComfyCubePreviewCatalog } from '../../frontend/comfyui/ui/surface/ComfyCubePreviewCatalog.js';
import { CubePreviewRetentionStore } from '../../frontend/comfyui/ui/surface/CubePreviewRetentionStore.js';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { buildCubeOutputExecutionId } from '../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';

describe('ComfyCubePreviewCatalog', () => {
  test('collects only media belonging to a Cube output boundary', () => {
    const outputNode = { id: 'save', type: 'PreviewImage', title: 'Saved image' };
    const sampler = { id: 'sampler', type: 'KSampler', title: 'Sampler' };
    const cube = container('definition', [outputNode, sampler], ['image'], outputNode);
    const catalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => ({
        'container-definition:save': {
          images: [{ filename: 'output.png', subfolder: '', type: 'output' }],
        },
      }),
      getNodePreviewImages: () => ({
        'container-definition:sampler': ['blob:live-sample'],
      }),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube)).toEqual({
      outputs: [
        {
          id: 'image',
          label: 'image',
          items: [
            {
              key: 'container-definition:save:output:0',
              url: '/view?filename=output.png',
              label: 'image',
              sourceLocator: 'container-definition:save',
            },
          ],
        },
      ],
    });
  });

  test('does not route transient internal upload previews into the Cube output rail', () => {
    const maskLoader = {
      id: 'load-mask-batch',
      type: 'SimpleSyrup.LoadMaskBatch',
      title: 'Load Mask Batch',
    };
    const cube = container('definition', [maskLoader], []);
    const catalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => ({
        'load-mask-batch': {
          images: [{ filename: 'selected-mask.png', subfolder: '', type: 'temp' }],
        },
      }),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube)).toEqual({ outputs: [] });
  });

  test('reads the selected boundary preview from its restored CubeOutput execution sink', () => {
    const outputNode = { id: 'decode', type: 'VAEDecode', title: 'Decode' };
    const cube = container('definition', [outputNode], ['output.image'], outputNode);
    const sinkId = buildCubeOutputExecutionId(cube.id, 0);
    const catalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => ({
        [sinkId]: {
          images: [{ filename: 'cube-output.png', subfolder: '', type: 'temp' }],
        },
      }),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube).outputs[0]?.items).toEqual([
      {
        key: `${sinkId}:output:0`,
        url: '/view?filename=cube-output.png',
        label: 'output.image',
        sourceLocator: sinkId,
      },
    ]);
  });

  test('reads a prompt-only CubeOutput result captured outside Comfy nodeOutputs', () => {
    const outputNode = { id: 'decode', type: 'VAEDecode', title: 'Decode' };
    const cube = container('definition', [outputNode], ['output.image'], outputNode);
    const sinkId = buildCubeOutputExecutionId(cube.id, 0);
    const capturedOutputs: Record<string, unknown> = {
      [sinkId]: {
        images: [{ filename: 'captured-result.png', subfolder: '', type: 'temp' }],
      },
    };
    const catalog = new ComfyCubePreviewCatalog({
      getCubeOutput: (executionId) => capturedOutputs[executionId],
      getNodeOutputs: () => ({}),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube).outputs[0]?.items).toEqual([
      {
        key: `${sinkId}:output:0`,
        url: '/view?filename=captured-result.png',
        label: 'output.image',
        sourceLocator: sinkId,
      },
    ]);
  });

  test('reads replacement Comfy output maps on every snapshot', () => {
    const outputNode = { id: 'save', type: 'PreviewImage' };
    const cube = container('definition', [outputNode], ['image'], outputNode);
    let nodeOutputs: Record<string, unknown> = {};
    const catalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => nodeOutputs,
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube).outputs[0]?.items).toEqual([]);
    nodeOutputs = {
      'container-definition:save': {
        images: [{ filename: 'replacement.png', subfolder: '', type: 'output' }],
      },
    };

    expect(catalog.snapshot(cube).outputs[0]?.items[0]?.url).toBe('/view?filename=replacement.png');
  });

  test('prefers durable execution output over a transient live-preview URL', () => {
    const outputNode = { id: 'save', type: 'PreviewImage' };
    const cube = container('definition', [outputNode], ['image'], outputNode);
    const catalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => ({
        'container-definition:save': {
          images: [{ filename: 'durable.png', subfolder: '', type: 'temp' }],
        },
      }),
      getNodePreviewImages: () => ({
        'container-definition:save': ['blob:transient-preview'],
      }),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube).outputs[0]?.items[0]?.url).toBe('/view?filename=durable.png');
  });

  test('keeps Comfy view URLs stable across host cache-buster refreshes', () => {
    const outputNode = { id: 'save', type: 'PreviewImage' };
    const cube = container('definition', [outputNode], ['image'], outputNode);
    let refresh = 0;
    const catalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => ({
        'container-definition:save': { images: [{ filename: 'stable.png', type: 'temp' }] },
      }),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: () =>
        `/api/view?filename=stable.png&type=temp&rand=${String(++refresh)}`,
    });

    const first = catalog.snapshot(cube).outputs[0]?.items[0]?.url;
    const second = catalog.snapshot(cube).outputs[0]?.items[0]?.url;

    expect(first).toBe('/api/view?filename=stable.png&type=temp');
    expect(second).toBe(first);
  });

  test('reads outputs by the flattened Cube instance path Comfy stores', () => {
    const outputNode = { id: 'save', type: 'PreviewImage' };
    const cube = container('definition', [outputNode], ['image'], outputNode);
    const catalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => ({
        'container-definition:save': {
          images: [{ filename: 'executed.png', subfolder: '', type: 'output' }],
        },
      }),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube).outputs[0]?.items[0]?.url).toBe('/view?filename=executed.png');
  });

  test('uses the Cube instance node identity instead of the subgraph definition identity', () => {
    const outputNode = {
      id: 'save',
      type: 'PreviewImage',
    };
    const cube = container('serialized-definition', [outputNode], ['image'], outputNode);
    cube.subgraph.id = 'live-definition';
    const catalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => ({
        'container-serialized-definition:save': {
          images: [{ filename: 'live.png', subfolder: '', type: 'output' }],
        },
      }),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube).outputs[0]?.items[0]?.url).toBe('/view?filename=live.png');
  });

  test('does not route nested internal previews into the Cube output rail', () => {
    const nestedSampler = { id: 'sampler', type: 'KSampler', title: 'Nested sampler' };
    const nestedSubgraph = {
      id: 'nested',
      type: 'nested-definition',
      subgraph: { id: 'nested-definition', _nodes: [nestedSampler] },
    };
    const cube = container('cube-definition', [nestedSubgraph], []);
    const catalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => ({}),
      getNodePreviewImages: () => ({
        'container-cube-definition:nested:sampler': ['blob:nested-sample'],
      }),
      buildOutputImageUrl: () => null,
    });

    expect(catalog.snapshot(cube)).toEqual({ outputs: [] });
  });

  test('uses the nearest downstream native media output for a Cube boundary', () => {
    const outputNode = { id: 'decode', type: 'VAEDecode' };
    const cube = container('definition', [outputNode], ['image'], outputNode);
    cube.outputs = [{ name: 'image', type: 'IMAGE', links: ['cube-to-pass'] }];
    const pass = {
      id: 'pass',
      inputs: [{ link: 'cube-to-pass' }],
      outputs: [{ links: ['pass-to-preview'] }],
    };
    const preview = {
      id: 'preview',
      title: 'Preview Image',
      inputs: [{ link: 'pass-to-preview' }],
      outputs: [],
    };
    const rootGraph = {
      _nodes: [cube, pass, preview],
      links: {
        'cube-to-pass': {
          id: 'cube-to-pass',
          origin_id: cube.id,
          origin_slot: 0,
          target_id: pass.id,
          target_slot: 0,
        },
        'pass-to-preview': {
          id: 'pass-to-preview',
          origin_id: pass.id,
          origin_slot: 0,
          target_id: preview.id,
          target_slot: 0,
        },
      },
    };
    const catalog = new ComfyCubePreviewCatalog({
      getRootGraph: () => rootGraph,
      getNodeOutputs: () => ({
        preview: {
          images: [{ filename: 'downstream.png', subfolder: '', type: 'temp' }],
        },
      }),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube).outputs[0]?.items).toEqual([
      {
        key: 'preview:output:0',
        url: '/view?filename=downstream.png',
        label: 'image',
        sourceLocator: 'preview',
      },
    ]);
  });

  test('follows dotted auto-connect routes when resolving a Cube output preview', () => {
    const outputNode = { id: 'decode', type: 'VAEDecode' };
    const cube = container('definition', [outputNode], ['image'], outputNode);
    cube.outputs = [{ name: 'image', type: 'IMAGE', links: null }];
    const preview = { id: 'preview', title: 'Preview Image', inputs: [], outputs: [] };
    const catalog = new ComfyCubePreviewCatalog({
      getRootGraph: () => ({ _nodes: [cube, preview], links: {} }),
      getEffectiveLinks: () => [{ originId: cube.id, originSlot: 0, targetId: preview.id }],
      getNodeOutputs: () => ({
        preview: {
          images: [{ filename: 'auto-connected.png', subfolder: '', type: 'temp' }],
        },
      }),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube).outputs[0]?.items[0]?.url).toBe(
      '/view?filename=auto-connected.png',
    );
  });

  test('retains the last executed media across transient graph lifecycle resets', () => {
    const outputNode = { id: 'decode', type: 'VAEDecode' };
    const cube = container('definition', [outputNode], ['image'], outputNode);
    cube.outputs = [{ name: 'image', type: 'IMAGE', links: null }];
    const preview = { id: 'preview', title: 'Preview Image', inputs: [], outputs: [] };
    let effectiveLinks = [{ originId: cube.id, originSlot: 0, targetId: preview.id }];
    const catalog = new ComfyCubePreviewCatalog({
      getRootGraph: () => ({ _nodes: [cube, preview], links: {} }),
      getEffectiveLinks: () => effectiveLinks,
      getNodeOutputs: () => ({
        preview: {
          images: [{ filename: 'retained.png', subfolder: '', type: 'temp' }],
        },
      }),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(cube).outputs[0]?.items[0]?.url).toBe('/view?filename=retained.png');
    effectiveLinks = [];

    expect(catalog.snapshot(cube).outputs[0]?.items[0]?.url).toBe('/view?filename=retained.png');
  });

  test('retains executed media when Comfy replaces the Cube node object', () => {
    const outputNode = { id: 'decode', type: 'VAEDecode' };
    const original = container('definition', [outputNode], ['image'], outputNode);
    let nodeOutputs: Record<string, unknown> = {
      'container-definition:decode': {
        images: [{ filename: 'retained-after-navigation.png', subfolder: '', type: 'temp' }],
      },
    };
    const catalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => nodeOutputs,
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(catalog.snapshot(original).outputs[0]?.items[0]?.url).toBe(
      '/view?filename=retained-after-navigation.png',
    );
    nodeOutputs = {};
    const replacementOutput = { id: 'decode', type: 'VAEDecode' };
    const replacement = container('definition', [replacementOutput], ['image'], replacementOutput);

    expect(catalog.snapshot(replacement).outputs[0]?.items[0]?.url).toBe(
      '/view?filename=retained-after-navigation.png',
    );
  });

  test('retains executed media when Comfy reconstructs graph-bound Cube collaborators', () => {
    const rootGraph = { _nodes: [], links: {} };
    const outputNode = { id: 'decode', type: 'VAEDecode' };
    const original = container('definition', [outputNode], ['image'], outputNode);
    const retention = new CubePreviewRetentionStore();
    const populated = new ComfyCubePreviewCatalog({
      getRootGraph: () => rootGraph,
      retention,
      getNodeOutputs: () => ({
        'container-definition:decode': {
          images: [{ filename: 'retained-after-runtime-reset.png', type: 'temp' }],
        },
      }),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: (image) => `/view?filename=${String(image.filename)}`,
    });

    expect(populated.snapshot(original).outputs[0]?.items[0]?.url).toBe(
      '/view?filename=retained-after-runtime-reset.png',
    );

    const replacementOutput = { id: 'decode', type: 'VAEDecode' };
    const replacement = container('definition', [replacementOutput], ['image'], replacementOutput);
    const reconstructed = new ComfyCubePreviewCatalog({
      getRootGraph: () => rootGraph,
      retention,
      getNodeOutputs: () => ({}),
      getNodePreviewImages: () => ({}),
      buildOutputImageUrl: () => null,
    });

    expect(reconstructed.snapshot(replacement).outputs[0]?.items[0]?.url).toBe(
      '/view?filename=retained-after-runtime-reset.png',
    );
  });
});

/** Build a real Cube node with native output-boundary resolution. */
function container(
  id: string,
  nodes: object[],
  outputNames: string[],
  outputNode?: object,
): CubeNode {
  const boundaryLink = {
    resolve: () => ({ outputNode }),
  };
  const subgraph = {
    id,
    name: id,
    _nodes: nodes,
    inputs: [],
    outputs: outputNames.map((name) => ({ name, type: 'IMAGE' })),
    inputNode: {},
    outputNode: {
      slots: outputNames.map(() => ({ getLinks: () => [boundaryLink] })),
    },
    add() {},
    remove() {},
    addInput() {
      throw new Error('not used');
    },
    addOutput() {
      throw new Error('not used');
    },
    configure() {},
  } as unknown as NativeCubeSubgraph;
  return {
    id: `container-${id}`,
    type: id,
    title: id,
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: `instance-${id}`, cube_id: `${id}.cube` },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
