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
/** Verify Nodes 1.0 Cube chrome reuses native and PrimeIcons drawing primitives. */

import { jest } from '@jest/globals';
import {
  requireCubeIdentity,
  type CubeNode,
} from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeIconResolver } from '../../frontend/comfyui/ui/core/CubeIconResolver.js';
import { computeCubeCanvasLayout } from '../../frontend/comfyui/ui/surface/CubeCanvasLayout.js';
import { CubeCanvasChromeRenderer } from '../../frontend/comfyui/ui/surface/CubeCanvasChromeRenderer.js';
import { comfyPrimeIconGlyph } from '../../frontend/comfyui/ui/surface/ComfyPrimeIcons.js';
import { createDefaultCubeSurfaceState } from '../../frontend/comfyui/ui/surface/CubeSurfaceState.js';

describe('CubeCanvasChromeRenderer', () => {
  test('draws icon-only actions beside Comfy native editor button without custom outlines', () => {
    const inner = {
      id: 'inner',
      type: 'OptionalPatch',
      mode: 4,
      pos: [0, 0],
      size: [240, 100],
      inputs: [],
      outputs: [],
      widgets: [],
      properties: {},
      connect() {},
    };
    const node = cubeNode(inner);
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = false;
    const layout = computeCubeCanvasLayout(node, state, 30, ['swap-left', 'swap-right']);
    const context = drawingContext();
    const editorButton = {
      name: 'enter_subgraph',
      height: 14,
      visible: true,
      xOffset: -10,
      yOffset: 0,
      getWidth: jest.fn(() => 16),
      draw: jest.fn(),
    };

    new CubeCanvasChromeRenderer(new CubeIconResolver()).draw(context, {
      node,
      layout,
      chromeActions: {
        onSwapLeft() {},
        onSwapRight() {},
      },
      editorButton,
      headerColor: '#333',
      titleTextColor: '#f0f2f5',
    });

    expect(editorButton.draw).toHaveBeenCalledWith(context, expect.any(Number), expect.any(Number));
    const texts = (
      context.fillText as jest.MockedFunction<CanvasRenderingContext2D['fillText']>
    ).mock.calls.map(([text]) => text);
    expect(texts).toEqual(
      expect.arrayContaining([
        'from Base-Cubes by Artificial-Sweetener',
        'SDXL',
        'Text to Image version 2.0.0',
        comfyPrimeIconGlyph('arrow-left'),
        comfyPrimeIconGlyph('arrow-right'),
      ]),
    );
    expect(texts).not.toEqual(
      expect.arrayContaining([comfyPrimeIconGlyph('eye'), comfyPrimeIconGlyph('box')]),
    );
    expect(texts).not.toEqual(
      expect.arrayContaining([comfyPrimeIconGlyph('save'), comfyPrimeIconGlyph('ban')]),
    );
    expect(texts).not.toEqual(expect.arrayContaining(['⇦', '⇨', '▱', '◉']));
    expect(context.stroke).not.toHaveBeenCalled();
    expect(context.roundRect).toHaveBeenCalledWith(
      expect.any(Number),
      expect.any(Number),
      expect.any(Number),
      expect.any(Number),
      expect.any(Number),
    );
  });

  test('draws the crossed-out save mark only for a Cube awaiting its first save', () => {
    const inner = {
      id: 'inner',
      type: 'OptionalPatch',
      mode: 4,
      pos: [0, 0],
      size: [240, 100],
      inputs: [],
      outputs: [],
      widgets: [],
      properties: {},
      connect() {},
    };
    const node = cubeNode(inner);
    node.properties.sugarcubes_kind = 'cube_draft';
    delete requireCubeIdentity(node).cube_id;
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = false;
    const layout = computeCubeCanvasLayout(node, state, 30);
    const context = drawingContext();

    new CubeCanvasChromeRenderer(new CubeIconResolver()).draw(context, {
      node,
      layout,
      chromeActions: null,
      editorButton: null,
      headerColor: '#333',
      titleTextColor: '#f0f2f5',
    });

    const texts = (
      context.fillText as jest.MockedFunction<CanvasRenderingContext2D['fillText']>
    ).mock.calls.map(([text]) => text);
    expect(texts).toEqual(
      expect.arrayContaining([
        'Workflow only',
        comfyPrimeIconGlyph('save'),
        comfyPrimeIconGlyph('ban'),
      ]),
    );
    expect(texts).not.toEqual(expect.arrayContaining(['Saved', 'Unsaved']));
  });
});

/** Build one native SubgraphNode-shaped Cube for header composition. */
function cubeNode(inner: CubeNode['subgraph']['_nodes'][number]): CubeNode {
  return {
    id: 'cube-node',
    type: 'cube-definition',
    title: 'Text to Image',
    pos: [100, 140],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        instance_id: 'cube-instance',
        cube_id: 'Artificial-Sweetener/Base-Cubes/Text to Image.cube',
        default_alias: 'SDXL/Text to Image',
        target_model: 'SDXL',
        cube_version: 'v2.0.0',
      },
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

/** Provide the finite canvas surface used by header composition. */
function drawingContext(): CanvasRenderingContext2D {
  return {
    save: jest.fn(),
    restore: jest.fn(),
    translate: jest.fn(),
    scale: jest.fn(),
    fillRect: jest.fn(),
    beginPath: jest.fn(),
    fill: jest.fn(),
    fillText: jest.fn(),
    measureText: jest.fn(
      () =>
        ({
          width: 12,
          actualBoundingBoxAscent: 9,
          actualBoundingBoxDescent: 3,
          actualBoundingBoxLeft: 0,
          actualBoundingBoxRight: 12,
        }) as TextMetrics,
    ),
    stroke: jest.fn(),
    roundRect: jest.fn(),
  } as unknown as CanvasRenderingContext2D;
}
