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
/** Verify Nodes 1.0 Cube masonry measures the slotless native presentation. */

import { jest } from '@jest/globals';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { computeCubeCanvasLayout } from '../../frontend/comfyui/ui/surface/CubeCanvasLayout.js';
import { layoutCubeCanvasChrome } from '../../frontend/comfyui/ui/surface/CubeCanvasChromeLayout.js';
import { createDefaultCubeSurfaceState } from '../../frontend/comfyui/ui/surface/CubeSurfaceState.js';

describe('computeCubeCanvasLayout', () => {
  test('measures native widgets without changing internal graph state', () => {
    const inputs = [{ name: 'model' }, { name: 'positive' }, { name: 'negative' }];
    const outputs = [{ name: 'LATENT' }];
    const flags = { collapsed: true, pinned: true };
    const inner = {
      id: 'inner',
      type: 'KSampler',
      pos: [0, 0],
      size: [320, 260],
      inputs,
      outputs,
      flags,
      widgets: [{ name: 'steps', computeSize: jest.fn(() => [240, 78]) }],
      widgets_up: false,
      properties: {},
      connect() {},
      computeSize: jest.fn(() => [240, 260]),
    };
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = false;
    const cube = cubeNode(inner);

    const layout = computeCubeCanvasLayout(cube, state, 30);

    expect(layout.cards).toHaveLength(1);
    expect(layout.cards[0]?.bodyHeight).toBe(96);
    expect(layout.cards[0]?.rect.height).toBe(126);
    expect(layout.cards[0]?.rect.y).toBe(layout.header.y + layout.header.height + state.gap);
    expect(layout.minimumSize).toEqual([440, 180]);
    expect(inner.inputs).toBe(inputs);
    expect(inner.outputs).toBe(outputs);
    expect(inner.flags).toBe(flags);
    expect(inner.widgets_up).toBe(false);
    expect(inner.size).toEqual([320, 260]);
    expect(inner.computeSize).not.toHaveBeenCalled();
  });

  test('derives minimum height from the tallest responsive masonry column and both gutters', () => {
    const state = createDefaultCubeSurfaceState();
    state.gap = 20;
    state.minimumColumnWidth = 300;
    state.preview.visible = false;
    const innerNodes = Array.from({ length: 3 }, (_, index) => ({
      id: `inner-${String(index)}`,
      type: `Node${String(index)}`,
      pos: [0, index * 120],
      size: [300, 100],
      widgets: [{ name: 'value', computeSize: () => [240, 80] }],
      inputs: [],
      outputs: [],
      properties: {},
      connect() {},
    }));
    const cube = cubeNode(innerNodes[0] as CubeNode['subgraph']['_nodes'][number]);
    cube.subgraph._nodes = innerNodes;
    cube.size[0] = 360;

    const layout = computeCubeCanvasLayout(cube, state, 30);

    expect(layout.cards.map((card) => card.rect.height)).toEqual([128, 128, 128]);
    expect(layout.minimumSize).toEqual([440, 476]);
  });

  test('aligns output ports with equal preview-section title rows', () => {
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = true;
    state.preview.width = 300;
    const cube = cubeNode({
      id: 'inner',
      type: 'PreviewImage',
      pos: [0, 0],
      size: [240, 100],
      inputs: [],
      outputs: [],
      widgets: [],
      properties: {},
      connect() {},
    });
    const firstOutput = { name: 'output.image', type: 'IMAGE' };
    const secondOutput = { name: 'output.mask', type: 'MASK' };
    cube.outputs = [firstOutput, secondOutput];
    cube.subgraph.outputs = [firstOutput, secondOutput];

    const layout = computeCubeCanvasLayout(cube, state, 30);

    expect(layout.preview).not.toBeNull();
    expect(layout.outputs.map((output) => output.slot)).toEqual([firstOutput, secondOutput]);
    expect(layout.outputs.map((output) => output.y)).toEqual([180, 400]);
    expect(
      layout.outputs.every((output) => output.x === layout.frame.x + layout.frame.width - 9),
    ).toBe(true);
    expect(layout.inputGutter.width).toBe(0);
    expect(layout.outputGutter.width).toBe(18);
    expect(layout.preview?.x).toBeGreaterThanOrEqual(
      layout.inputGutter.x + layout.inputGutter.width,
    );
    expect((layout.preview?.x ?? 0) + (layout.preview?.width ?? 0)).toBe(
      layout.frame.x + layout.frame.width,
    );
    expect(
      layout.outputs.every(
        (output) =>
          output.minY <= output.y && output.y <= output.maxY && output.labelY === output.y,
      ),
    ).toBe(true);
  });

  test('packs input ports from the native top row instead of centering them on the Cube', () => {
    const state = createDefaultCubeSurfaceState();
    const cube = cubeNode({
      id: 'inner',
      type: 'Upscale',
      pos: [0, 0],
      size: [240, 100],
      inputs: [],
      outputs: [],
      widgets: [],
      properties: {},
      connect() {},
    });
    const imageInput = { name: 'image', type: 'IMAGE' };
    cube.inputs = [imageInput];
    cube.subgraph.inputs = [imageInput];

    const layout = computeCubeCanvasLayout(cube, state, 30);

    expect(layout.inputs[0]?.y).toBe(layout.inputs[0]?.minY);
    expect(layout.inputs[0]?.y).toBeLessThan(layout.frame.y + layout.frame.height / 2);
  });

  test('does not constrain a previewless output to its resize-dependent default anchor', () => {
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = false;
    const cube = cubeNode({
      id: 'inner',
      type: 'Producer',
      pos: [0, 0],
      size: [240, 100],
      inputs: [],
      outputs: [],
      widgets: [],
      properties: {},
      connect() {},
    });
    const imageOutput = { name: 'image', type: 'IMAGE' };
    cube.outputs = [imageOutput];
    cube.subgraph.outputs = [imageOutput];
    cube.size[1] = 600;

    const layout = computeCubeCanvasLayout(cube, state, 30);
    const output = layout.outputs[0];

    expect(layout.preview).toBeNull();
    expect(output?.defaultY).toBeGreaterThan(output?.minY ?? Number.POSITIVE_INFINITY);
    expect(output?.labelY).toBe(output?.minY);
  });

  test('does not reserve either boundary affordance when the Cube has no ports', () => {
    const state = createDefaultCubeSurfaceState();
    const cube = cubeNode({
      id: 'inner',
      type: 'PreviewImage',
      pos: [0, 0],
      size: [240, 100],
      inputs: [],
      outputs: [],
      widgets: [],
      properties: {},
      connect() {},
    });
    cube.inputs = [];
    cube.outputs = [];
    cube.subgraph.inputs = [];
    cube.subgraph.outputs = [];

    const layout = computeCubeCanvasLayout(cube, state, 30);

    expect(layout.inputGutter.width).toBe(0);
    expect(layout.outputGutter.width).toBe(0);
    expect(layout.content.x).toBe(layout.frame.x + 12);
    expect(layout.content.x + layout.content.width).toBe(layout.frame.x + layout.frame.width - 12);
  });

  test('does not expose dormant native draft boundaries on the closed face', () => {
    const state = createDefaultCubeSurfaceState();
    const cube = cubeNode({
      id: 'inner',
      type: 'Draft content',
      pos: [0, 0],
      size: [240, 100],
      inputs: [],
      outputs: [],
      widgets: [],
      properties: {},
      connect() {},
    });
    cube.inputs = [{ name: 'input', type: '*' }];
    cube.outputs = [{ name: 'output', type: '*' }];

    const layout = computeCubeCanvasLayout(cube, state, 30, [], {
      inputSlots: [],
      outputSlots: [],
    });

    expect(layout.inputs).toEqual([]);
    expect(layout.outputs).toEqual([]);
    expect(layout.preview).toBeNull();
    expect(layout.inputGutter.width).toBe(0);
    expect(layout.outputGutter.width).toBe(0);
  });

  test('reserves a compact hit target for the native Nodes 1.0 SubgraphNode editor button', () => {
    const state = createDefaultCubeSurfaceState();
    const cube = cubeNode({
      id: 'inner',
      type: 'PreviewImage',
      pos: [0, 0],
      size: [240, 100],
      inputs: [],
      outputs: [],
      widgets: [],
      properties: {},
      connect() {},
    });

    const layout = computeCubeCanvasLayout(cube, state, 30);
    expect(layout.editAction.width).toBe(28);
    expect(layout.editAction.height).toBe(24);
  });

  test('packs only visible Nodes 1.0 actions and keeps the native editor action farthest right', () => {
    const chrome = layoutCubeCanvasChrome(
      { x: 100, y: 110, width: 720, height: 42 },
      {
        showCardMenu: true,
        showUnsavedIndicator: false,
        titlebarActionKeys: ['swap-left', 'cube-menu'],
      },
    );

    expect(chrome.chromeActions['swap-right']).toBeUndefined();
    expect(chrome.editAction.x + chrome.editAction.width).toBe(808);
    expect(chrome.chromeActions['cube-menu']?.x).toBe(chrome.editAction.x - 6 - 28);
    expect(chrome.chromeActions['swap-left']?.x).toBe(
      (chrome.chromeActions['cube-menu']?.x ?? 0) - 6 - 28,
    );
    expect(chrome.cardMenuAction.x).toBe((chrome.chromeActions['swap-left']?.x ?? 0) - 6 - 28);
  });

  test('reserves a titlebar corner slot only for a Cube awaiting its first save', () => {
    const saved = layoutCubeCanvasChrome(
      { x: 100, y: 110, width: 720, height: 42 },
      {
        showCardMenu: false,
        showUnsavedIndicator: false,
        titlebarActionKeys: [],
      },
    );
    const draft = layoutCubeCanvasChrome(
      { x: 100, y: 110, width: 720, height: 42 },
      {
        showCardMenu: false,
        showUnsavedIndicator: true,
        titlebarActionKeys: [],
      },
    );

    expect(saved.unsavedIndicator).toBeNull();
    expect(draft.unsavedIndicator?.x).toBe(draft.editAction.x - 6 - 28);
  });
});

/** Build the real Cube-node boundary required by canvas layout. */
function cubeNode(inner: CubeNode['subgraph']['_nodes'][number]): CubeNode {
  return {
    id: 'cube-node',
    type: 'cube-definition',
    title: 'Text to Image',
    pos: [100, 140],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-instance' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'cube-definition',
      name: 'Text to Image',
      _nodes: [inner],
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      addInput() {
        throw new Error('not used');
      },
      addOutput() {
        throw new Error('not used');
      },
      configure() {},
    },
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
    setSize(size) {
      this.size[0] = size[0] ?? 0;
      this.size[1] = size[1] ?? 0;
    },
  };
}
