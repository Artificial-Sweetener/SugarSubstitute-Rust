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
/** Characterize exact Nodes 1.0 rendering under Cube-face presentation. */

import { jest } from '@jest/globals';
import {
  ComfyLiteGraphNodeCardRenderer,
  drawNativeLiteGraphCubeCard,
} from '../../frontend/comfyui/ui/surface/ComfyLiteGraphNodeCardRenderer.js';

describe('ComfyLiteGraphNodeCardRenderer', () => {
  test('passes the exact internal node to Comfy and restores masked host callbacks', () => {
    const drawSlots = jest.fn();
    const drawCollapsedSlots = jest.fn();
    const onDrawBackground = jest.fn();
    const titleButtons = [{ name: 'enter_subgraph' }];
    const imgs = [{ src: 'preview.png' }];
    const animatedImages = [{ src: 'animated.webp' }];
    const updateArea = jest.fn();
    const strokeStyles: Record<string, () => object | undefined> = {
      selected: () => undefined,
    };
    const node = {
      id: 'inside',
      type: 'KSampler',
      size: [240, 180],
      drawSlots,
      drawCollapsedSlots,
      onDrawBackground,
      title_buttons: titleButtons,
      imgs,
      animatedImages,
      imageIndex: 2,
      strokeStyles,
      updateArea,
    };
    const context = {
      setTransform: jest.fn(),
      clearRect: jest.fn(),
      save: jest.fn(),
      translate: jest.fn(),
      restore: jest.fn(),
    } as unknown as CanvasRenderingContext2D;
    const getContext = jest
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue(context);
    const drawNode = jest.fn((drawnNode: typeof node) => {
      expect(drawnNode).toBe(node);
      expect(drawnNode.drawSlots).not.toBe(drawSlots);
      expect(drawnNode.onDrawBackground).toBe(onDrawBackground);
      expect(drawnNode.title_buttons).toEqual([]);
      expect(drawnNode.imgs).toEqual([]);
      expect(drawnNode.animatedImages).toEqual([]);
      expect(drawnNode.imageIndex).toBeNull();
      expect(drawnNode.strokeStyles).toBe(strokeStyles);
      expect(drawnNode.strokeStyles.sugarcubesCubeFace).toBeUndefined();
    });
    const renderer = new ComfyLiteGraphNodeCardRenderer({
      document,
      canvasRenderer: { drawNode },
      titleHeight: 42,
      devicePixelRatio: () => 2,
    });
    const target = document.createElement('div');

    renderer.mount(target, node);

    expect(drawNode).toHaveBeenCalledWith(node, context);
    expect(updateArea).toHaveBeenCalledWith(context);
    expect(node.drawSlots).toBe(drawSlots);
    expect(node.drawCollapsedSlots).toBe(drawCollapsedSlots);
    expect(node.onDrawBackground).toBe(onDrawBackground);
    expect(node.title_buttons).toBe(titleButtons);
    expect(node.imgs).toBe(imgs);
    expect(node.animatedImages).toBe(animatedImages);
    expect(node.imageIndex).toBe(2);
    expect(node.strokeStyles).toBe(strokeStyles);
    expect(node.strokeStyles.sugarcubesCubeFace).toBeUndefined();
    expect(target.querySelector('[data-cube-face-native="nodes-1"]')).not.toBeNull();
    expect(
      target.querySelector<HTMLCanvasElement>('[data-cube-face-native="nodes-1"]')?.style
        .aspectRatio,
    ).toBe('240 / 43');
    getContext.mockRestore();
  });

  test('isolates face rendering from the mutable active graph zoom', () => {
    const node = {
      id: 'inside',
      type: 'KSampler',
      size: [240, 180],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
    };
    const context = {
      setTransform: jest.fn(),
      clearRect: jest.fn(),
      save: jest.fn(),
      translate: jest.fn(),
      restore: jest.fn(),
    } as unknown as CanvasRenderingContext2D;
    const getContext = jest
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue(context);
    const canvasRenderer = {
      ds: { scale: 1.8 },
      drawNode: jest.fn(() => {
        expect(canvasRenderer.ds.scale).toBe(1);
      }),
    };
    const renderer = new ComfyLiteGraphNodeCardRenderer({
      document,
      canvasRenderer,
      titleHeight: 42,
      devicePixelRatio: () => 1,
    });

    renderer.mount(document.createElement('div'), node);

    expect(canvasRenderer.ds.scale).toBe(1.8);
    getContext.mockRestore();
  });

  test('lays out at a requested card width and restores internal graph geometry', () => {
    const updateArea = jest.fn();
    const node = {
      id: 'inside',
      type: 'Prompt',
      size: [420, 180],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
      updateArea,
    };
    const drawNode = jest.fn(() => {
      expect(node.size[0]).toBe(260);
    });
    const context = {} as CanvasRenderingContext2D;

    drawNativeLiteGraphCubeCard({ drawNode }, node, context, {
      presentationWidth: 260,
    });

    expect(node.size).toEqual([420, 180]);
    expect(updateArea).toHaveBeenNthCalledWith(1, context);
    expect(updateArea).toHaveBeenNthCalledWith(2, context);
  });

  test('applies one parent Cube theme only during the native card draw', () => {
    const node = {
      id: 'inside',
      type: 'KSampler',
      size: [240, 180],
      color: '#171718',
      bgcolor: '#262729',
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
    };
    const drawNode = jest.fn(() => {
      expect(node.color).toBe('#2b2859');
      expect(node.bgcolor).toBe('#202127');
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node, {} as CanvasRenderingContext2D, {
      theme: { header: '#2b2859', body: '#202127' },
    });

    expect(node.color).toBe('#171718');
    expect(node.bgcolor).toBe('#262729');
  });

  test('restores LiteGraph text baselines before Comfy draws titles and widgets', () => {
    const node = {
      id: 'inside',
      type: 'CheckpointLoaderSimple',
      size: [260, 120],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
    };
    const context = {
      textBaseline: 'middle',
    } as CanvasRenderingContext2D;
    const drawNode = jest.fn(() => {
      expect(context.textBaseline).toBe('alphabetic');
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node, context);

    expect(drawNode).toHaveBeenCalledWith(node, context);
  });

  test('renders a collapsed graph node expanded without changing its editor state', () => {
    const flags = { collapsed: true, pinned: true };
    const inputs = [{ name: 'model' }];
    const outputs = [{ name: 'LATENT' }];
    const node = {
      id: 'inside',
      type: 'KSampler',
      size: [240, 180],
      flags,
      inputs,
      outputs,
      widgets_start_y: 0,
      widgets_up: false,
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
      updateArea: jest.fn(),
    };
    const drawNode = jest.fn(() => {
      expect(node.flags).not.toBe(flags);
      expect(node.flags).toEqual({ collapsed: false, pinned: true });
      expect(node.inputs).toBe(inputs);
      expect(node.outputs).toBe(outputs);
      expect(node.widgets_start_y).toBe(22);
      expect(node.widgets_up).toBe(true);
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node, {} as CanvasRenderingContext2D);

    expect(node.flags).toBe(flags);
    expect(node.flags.collapsed).toBe(true);
    expect(node.inputs).toBe(inputs);
    expect(node.outputs).toBe(outputs);
    expect(node.widgets_start_y).toBe(0);
    expect(node.widgets_up).toBe(false);
  });

  test('uses a slotless native body height without changing graph geometry', () => {
    const inputs = [{ name: 'positive' }, { name: 'negative' }];
    const outputs = [{ name: 'LATENT' }];
    const node = {
      id: 'inside',
      type: 'KSampler',
      size: [320, 240],
      inputs,
      outputs,
      widgets_up: false,
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
      updateArea: jest.fn(),
    };
    const drawNode = jest.fn(() => {
      expect(node.size).toEqual([260, 96]);
      expect(node.inputs).toBe(inputs);
      expect(node.outputs).toBe(outputs);
      expect(node.widgets_up).toBe(true);
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node, {} as CanvasRenderingContext2D, {
      presentationWidth: 260,
      presentationHeight: 96,
    });

    expect(node.size).toEqual([320, 240]);
    expect(node.inputs).toBe(inputs);
    expect(node.outputs).toBe(outputs);
    expect(node.widgets_up).toBe(false);
  });

  test('restores inherited accessor-backed graph slots after a native face draw', () => {
    const inputs = [{ name: 'model' }];
    const outputs = [{ name: 'MODEL' }];
    let storedInputs = inputs;
    let storedOutputs = outputs;
    const prototype = {
      get inputs() {
        return storedInputs;
      },
      set inputs(value: typeof inputs) {
        storedInputs = value;
      },
      get outputs() {
        return storedOutputs;
      },
      set outputs(value: typeof outputs) {
        storedOutputs = value;
      },
    };
    const node = Object.assign(Object.create(prototype) as Record<string, unknown>, {
      id: 'inside',
      type: 'MahiroCFG',
      size: [240, 100],
      flags: {},
      widgets_up: false,
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
      updateArea: jest.fn(),
    });
    const drawNode = jest.fn(() => {
      expect(node.inputs).toBe(inputs);
      expect(node.outputs).toBe(outputs);
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node as never, {} as CanvasRenderingContext2D);

    expect(storedInputs).toBe(inputs);
    expect(storedOutputs).toBe(outputs);
    expect(Object.prototype.hasOwnProperty.call(node, 'inputs')).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(node, 'outputs')).toBe(false);
  });
});
