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
/** Verify Nodes 1.0 custom faces remain mounted on real graph nodes. */

import { jest } from '@jest/globals';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { ComfyLiteGraphCubeNodeHost } from '../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeNodeHost.js';
import type { CubeCanvasPreviewImageProvider } from '../../frontend/comfyui/ui/surface/CubeCanvasPreviewImageCache.js';

describe('ComfyLiteGraphCubeNodeHost', () => {
  test('replaces only the generic face hooks while native shell drawing remains owned by Comfy', () => {
    const rootGraph = {};
    const inner = {
      id: 'inner',
      type: 'KSampler',
      pos: [0, 0],
      size: [240, 180],
      properties: {},
      inputs: [],
      outputs: [],
      connect() {},
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawBackground: jest.fn(),
      color: '#171718',
      bgcolor: '#262729',
      title_buttons: [{ name: 'inner-action' }],
      strokeStyles: {} as Record<string, () => object | undefined>,
      updateArea: jest.fn(),
      widgets: [{ name: 'steps' }] as Array<{ name: string; [key: string]: unknown }>,
    };
    const node = cubeNode(inner);
    node.color = '#2b2859';
    node.bgcolor = '#202127';
    const originalForeground = jest.fn();
    const originalWidgets = jest.fn();
    const nativeEditorButton = {
      name: 'enter_subgraph',
      height: 14,
      visible: true,
      xOffset: -10,
      yOffset: 0,
      getWidth: jest.fn(() => 16),
      draw: jest.fn(),
    };
    const originalTitleButtons = [nativeEditorButton];
    node.onDrawForeground = originalForeground;
    node.drawWidgets = originalWidgets;
    node.title_buttons = originalTitleButtons;
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const innerDrawSlots = inner.drawSlots;
    const innerBackground = inner.onDrawBackground;
    const innerTitleButtons = inner.title_buttons;
    const canvasElement = document.createElement('canvas');
    const drawNode = jest.fn((drawnNode: unknown) => {
      expect(drawnNode).toBe(inner);
      expect(inner.drawSlots).not.toBe(innerDrawSlots);
      expect(inner.onDrawBackground).toBe(innerBackground);
      expect(inner.title_buttons).toEqual([]);
      expect(inner.strokeStyles.sugarcubesCubeFace).toBeUndefined();
      expect(inner.color).toBe('#2b2859');
      expect(inner.bgcolor).toBe('#202127');
    });
    const canvas = {
      canvas: canvasElement,
      graph: rootGraph,
      graph_mouse: [0, 0],
      drawNode,
      processWidgetClick: jest.fn(),
      setDirty: jest.fn(),
    };
    const host = new ComfyLiteGraphCubeNodeHost({
      canvas,
      document,
      rootGraph,
      nodes,
      history: {},
      titleHeight: 30,
      openEditor: jest.fn(),
    });

    host.setEnabled(true);
    expect(node.onDrawForeground).not.toBe(originalForeground);
    expect(node.drawWidgets).not.toBe(originalWidgets);
    expect(node.title_buttons).toEqual([]);

    const context = drawingContext();
    node.onDrawForeground?.(context, canvas, canvasElement);

    expect(drawNode).toHaveBeenCalledWith(inner, expect.any(Object));
    expect(context.fillText).toHaveBeenCalledWith('TI', 0, 0);
    expect(context.fillText).toHaveBeenCalledWith(
      'SDXL/Text to Image version 2.0.0',
      expect.any(Number),
      expect.any(Number),
      expect.any(Number),
    );
    expect(context.fillText).toHaveBeenCalledWith(
      'from Base-Cubes by Artificial-Sweetener',
      expect.any(Number),
      expect.any(Number),
      expect.any(Number),
    );
    expect(nativeEditorButton.draw).toHaveBeenCalledWith(
      context,
      expect.any(Number),
      expect.any(Number),
    );
    expect(context.arc).not.toHaveBeenCalled();
    expect(inner.updateArea).toHaveBeenCalledWith(expect.any(Object));
    expect(inner.drawSlots).toBe(innerDrawSlots);
    expect(inner.onDrawBackground).toBe(innerBackground);
    expect(inner.title_buttons).toBe(innerTitleButtons);
    expect(inner.color).toBe('#171718');
    expect(inner.bgcolor).toBe('#262729');
    expect(recordedFillStyles(context)).toContain('rgb(22 23 27)');

    host.dispose();
    expect(node.onDrawForeground).toBe(originalForeground);
    expect(node.drawWidgets).toBe(originalWidgets);
    expect(node.title_buttons).toBe(originalTitleButtons);
  });

  test('disables the custom hook inside the Cube graph and restores it on return', () => {
    const rootGraph = {};
    const inner = nativeInnerNode();
    const node = cubeNode(inner);
    const originalForeground = jest.fn();
    node.onDrawForeground = originalForeground;
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const canvasElement = document.createElement('canvas');
    const canvas = {
      canvas: canvasElement,
      graph: rootGraph,
      graph_mouse: [0, 0],
      drawNode: jest.fn(),
      processWidgetClick: jest.fn(),
      setDirty: jest.fn(),
    };
    const host = new ComfyLiteGraphCubeNodeHost({
      canvas,
      document,
      rootGraph,
      nodes,
      history: {},
      titleHeight: 30,
      openEditor: jest.fn(),
    });

    host.setEnabled(true);
    expect(node.onDrawForeground).not.toBe(originalForeground);
    canvas.graph = node.subgraph;
    host.sync();
    expect(node.onDrawForeground).toBe(originalForeground);
    canvas.graph = rootGraph;
    host.sync();
    expect(node.onDrawForeground).not.toBe(originalForeground);

    host.dispose();
  });

  test('remounts after Comfy replaces Nodes 1 draw hooks and preserves the replacement', () => {
    const rootGraph = {};
    const node = cubeNode(nativeInnerNode());
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const replacementForeground = jest.fn();
    const canvasElement = document.createElement('canvas');
    const canvas = {
      canvas: canvasElement,
      graph: rootGraph,
      graph_mouse: [0, 0],
      drawNode: jest.fn(),
      processWidgetClick: jest.fn(),
      setDirty: jest.fn(),
    };
    const host = new ComfyLiteGraphCubeNodeHost({
      canvas,
      document,
      rootGraph,
      nodes,
      history: {},
      titleHeight: 30,
      openEditor: jest.fn(),
    });

    host.setEnabled(true);
    node.onDrawForeground = replacementForeground;
    host.sync();

    expect(node.onDrawForeground).not.toBe(replacementForeground);
    host.dispose();
    expect(node.onDrawForeground).toBe(replacementForeground);
  });

  test('mounts and restores a real multiline DOM widget across Cube editor navigation', () => {
    const rootGraph = {};
    const inner = nativeInnerNode();
    const textarea = document.createElement('textarea');
    const nativeOwner = document.createElement('div');
    nativeOwner.append(textarea);
    document.body.append(nativeOwner);
    inner.widgets.push({
      name: 'prompt',
      element: textarea,
      margin: 10,
    });
    const node = cubeNode(inner);
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const canvasElement = document.createElement('canvas');
    canvasElement.getBoundingClientRect = jest.fn(
      () =>
        ({
          left: 0,
          top: 0,
          right: 1000,
          bottom: 800,
          width: 1000,
          height: 800,
          x: 0,
          y: 0,
          toJSON: () => ({}),
        }) as DOMRect,
    );
    const canvas = {
      canvas: canvasElement,
      graph: rootGraph,
      graph_mouse: [0, 0],
      ds: { scale: 1, offset: [0, 0] },
      drawNode: jest.fn(() => {
        Object.assign(inner.widgets[0] ?? {}, {
          y: 20,
          computedHeight: 100,
        });
      }),
      processWidgetClick: jest.fn(),
      setDirty: jest.fn(),
    };
    const host = new ComfyLiteGraphCubeNodeHost({
      canvas,
      document,
      rootGraph,
      nodes,
      history: {},
      titleHeight: 30,
      openEditor: jest.fn(),
    });

    host.setEnabled(true);
    node.onDrawForeground?.(drawingContext(), canvas, canvasElement);
    expect(document.querySelector('[data-sugarcubes-cube-face-dom-widget] textarea')).toBe(
      textarea,
    );

    canvas.graph = node.subgraph;
    host.sync();
    expect(textarea.parentElement).toBe(nativeOwner);
    expect(document.querySelector('[data-sugarcubes-cube-face-dom-widgets]')).toBeNull();

    canvas.graph = rootGraph;
    host.sync();
    node.onDrawForeground?.(drawingContext(), canvas, canvasElement);
    expect(document.querySelector('[data-sugarcubes-cube-face-dom-widget] textarea')).toBe(
      textarea,
    );

    host.dispose();
    expect(textarea.parentElement).toBe(nativeOwner);
    nativeOwner.remove();
  });

  test('draws multiple outputs as horizontal segments without captions in Nodes 1.0', () => {
    const rootGraph = {};
    const node = cubeNode(nativeInnerNode());
    node.outputs = [{ name: 'output.image', type: 'IMAGE' }];
    node.properties.sugarcubes_surface = {
      schema: 1,
      revealed: true,
      node_order: [],
      minimum_column_width: 240,
      gap: 12,
      preview: { visible: true, width: 320, selected_output: 'output.image' },
    };
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const context = drawingContext();
    const image = document.createElement('img');
    Object.defineProperties(image, {
      naturalWidth: { value: 1440 },
      naturalHeight: { value: 2016 },
    });
    const previewImages: CubeCanvasPreviewImageProvider = {
      get: jest.fn(() => image),
    };
    const canvasElement = document.createElement('canvas');
    const canvas = {
      canvas: canvasElement,
      graph: rootGraph,
      graph_mouse: [0, 0],
      drawNode: jest.fn(),
      processWidgetClick: jest.fn(),
      setDirty: jest.fn(),
    };
    const host = new ComfyLiteGraphCubeNodeHost({
      canvas,
      document,
      rootGraph,
      nodes,
      history: {},
      titleHeight: 30,
      openEditor: jest.fn(),
      previewImages,
      previewCatalog: {
        snapshot: () => ({
          outputs: [
            {
              id: 'output.image',
              label: 'output.image',
              items: [
                {
                  key: 'preview',
                  url: '/api/view?filename=proof.png',
                  label: 'Output: output.image',
                  sourceLocator: 'preview-node',
                },
              ],
            },
            {
              id: 'output.mask',
              label: 'output.mask',
              items: [
                {
                  key: 'mask-preview',
                  url: '/api/view?filename=mask.png',
                  label: 'Output: output.mask',
                  sourceLocator: 'mask-node',
                },
              ],
            },
          ],
          internalItems: [],
        }),
      },
    });

    host.setEnabled(true);
    node.onDrawForeground?.(context, canvas, canvasElement);

    expect(previewImages.get).toHaveBeenCalledWith('/api/view?filename=proof.png', 'preview-node');
    expect(previewImages.get).toHaveBeenCalledWith('/api/view?filename=mask.png', 'mask-node');
    expect(context.drawImage).toHaveBeenCalledTimes(2);
    const drawCalls = (
      context.drawImage as jest.MockedFunction<CanvasRenderingContext2D['drawImage']>
    ).mock.calls;
    expect(Number(drawCalls[0]?.[1])).toBe(Number(drawCalls[1]?.[1]));
    expect(Number(drawCalls[0]?.[2])).toBeLessThan(Number(drawCalls[1]?.[2]));
    const visibleText = (
      context.fillText as jest.MockedFunction<CanvasRenderingContext2D['fillText']>
    ).mock.calls.map(([text]) => text);
    expect(visibleText).toContain('output.image');
    expect(visibleText).toContain('output.mask');
    expect(visibleText).not.toContain('Output');
    expect(visibleText).not.toContain('Output: output.image');
    expect(visibleText).not.toContain('Output: output.mask');
    const outputLabel = (
      context.fillText as jest.MockedFunction<CanvasRenderingContext2D['fillText']>
    ).mock.calls.find(([text]) => text === 'output.image');
    const outputLabelY = Number(outputLabel?.[2]);
    expect(Number.isFinite(outputLabelY)).toBe(true);
    expect(
      (context.moveTo as jest.MockedFunction<CanvasRenderingContext2D['moveTo']>).mock.calls,
    ).toContainEqual([expect.any(Number), outputLabelY]);

    host.dispose();
  });
});

/** Build one real SubgraphNode-shaped Cube with native draw hooks. */
function cubeNode(inner: ReturnType<typeof nativeInnerNode>): CubeNode & {
  onDrawForeground?: (
    context: CanvasRenderingContext2D,
    canvas: unknown,
    element: HTMLCanvasElement,
  ) => void;
  drawWidgets?: (context: CanvasRenderingContext2D, options: unknown) => void;
  title_buttons?: unknown[];
} {
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
        cube_version: 'v2.0.0',
      },
      sugarcubes_surface: {
        schema: 1,
        revealed: true,
        node_order: [],
        minimum_column_width: 240,
        gap: 12,
        preview: { visible: false, width: 320, selected_output: null },
      },
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

/** Build one internal node whose object identity reaches Comfy's renderer. */
function nativeInnerNode() {
  return {
    id: 'inner',
    type: 'KSampler',
    pos: [0, 0],
    size: [240, 180],
    properties: {},
    inputs: [],
    outputs: [],
    connect() {},
    drawSlots: jest.fn(),
    drawCollapsedSlots: jest.fn(),
    onDrawBackground: jest.fn(),
    title_buttons: [] as unknown[],
    strokeStyles: {} as Record<string, () => object | undefined>,
    updateArea: jest.fn(),
    widgets: [] as Array<{ name: string; [key: string]: unknown }>,
  };
}

/** Provide the finite Canvas 2D surface exercised by Cube composition. */
function drawingContext(): CanvasRenderingContext2D {
  const context = {
    save: jest.fn(),
    restore: jest.fn(),
    translate: jest.fn(),
    beginPath: jest.fn(),
    arc: jest.fn(),
    roundRect: jest.fn(),
    rect: jest.fn(),
    fill: jest.fn(),
    clip: jest.fn(),
    fillRect: jest.fn(),
    fillText: jest.fn(),
    stroke: jest.fn(),
    moveTo: jest.fn(),
    lineTo: jest.fn(),
    drawImage: jest.fn(),
    measureText: jest.fn(() => ({ width: 12 }) as TextMetrics),
    scale: jest.fn(),
  } as Record<string, unknown>;
  const fillStyles: string[] = [];
  Object.defineProperty(context, 'fillStyle', {
    configurable: true,
    get: () => fillStyles.at(-1) ?? '',
    set: (value: unknown) => fillStyles.push(String(value)),
  });
  context.sugarcubesFillStyles = fillStyles;
  return context as unknown as CanvasRenderingContext2D;
}

/** Return every fill assigned while the Cube and its native cards were drawn. */
function recordedFillStyles(context: CanvasRenderingContext2D): readonly string[] {
  return (context as unknown as { sugarcubesFillStyles: readonly string[] }).sugarcubesFillStyles;
}
