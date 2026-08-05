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
  test('preserves native preview media while restoring masked host callbacks', () => {
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
      expect(drawnNode.imgs).toBe(imgs);
      expect(drawnNode.animatedImages).toBe(animatedImages);
      expect(drawnNode.imageIndex).toBe(2);
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
    ).toBe('240 / 222');
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

  test('replaces the subgraph workflow glyph with the normal three-line title decoration', () => {
    const subgraphTitleBox = jest.fn();
    const onDrawTitleBox = jest.fn();
    const subgraphNodePrototype = {
      drawTitleBox: subgraphTitleBox,
    };
    const node = Object.assign(Object.create(subgraphNodePrototype) as Record<string, unknown>, {
      id: 'nested',
      type: 'Subgraph',
      size: [240, 180],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawTitleBox,
      onDrawBackground: jest.fn(),
      title_buttons: [],
      skip_subgraph_button: false,
      strokeStyles: {},
      isSubgraphNode: () => true,
    });
    const drawNode = jest.fn((drawnNode: typeof node) => {
      expect(drawnNode.drawTitleBox).not.toBe(subgraphTitleBox);
      expect(drawnNode.onDrawTitleBox).toBe(onDrawTitleBox);
      expect(drawnNode.skip_subgraph_button).toBe(true);
      const context = {
        save: jest.fn(),
        restore: jest.fn(),
        beginPath: jest.fn(),
        moveTo: jest.fn(),
        lineTo: jest.fn(),
        stroke: jest.fn(),
      } as unknown as CanvasRenderingContext2D;
      const titleBox = drawnNode.drawTitleBox;
      if (typeof titleBox !== 'function') throw new Error('Expected a Cube-face title renderer.');
      titleBox(context, { title_height: 24 });
      expect(context.moveTo).toHaveBeenCalledTimes(3);
      expect(context.lineTo).toHaveBeenCalledTimes(3);
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node as never, {} as CanvasRenderingContext2D);

    expect(subgraphTitleBox).not.toHaveBeenCalled();
    expect(onDrawTitleBox).not.toHaveBeenCalled();
    expect(node.drawTitleBox).toBe(subgraphTitleBox);
    expect(node.onDrawTitleBox).toBe(onDrawTitleBox);
    expect(node.skip_subgraph_button).toBe(false);
  });

  test('leaves a normal card title box untouched', () => {
    const drawTitleBox = jest.fn();
    const onDrawTitleBox = jest.fn();
    const node = {
      id: 'regular',
      type: 'KSampler',
      size: [240, 180],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      drawTitleBox,
      onDrawTitleBox,
      onDrawBackground: jest.fn(),
      title_buttons: [],
      skip_subgraph_button: false,
      strokeStyles: {},
    };
    const drawNode = jest.fn((drawnNode: typeof node) => {
      expect(drawnNode.drawTitleBox).toBe(drawTitleBox);
      expect(drawnNode.onDrawTitleBox).toBe(onDrawTitleBox);
      expect(drawnNode.skip_subgraph_button).toBe(false);
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node, {} as CanvasRenderingContext2D);

    expect(node.drawTitleBox).toBe(drawTitleBox);
    expect(node.onDrawTitleBox).toBe(onDrawTitleBox);
    expect(node.skip_subgraph_button).toBe(false);
  });

  test('recognizes the native enter-subgraph title action when the host omits isSubgraphNode', () => {
    const drawTitleBox = jest.fn();
    const node = {
      id: 'nested',
      type: 'Subgraph',
      size: [240, 180],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      drawTitleBox,
      onDrawBackground: jest.fn(),
      title_buttons: [{ name: 'enter_subgraph' }],
      strokeStyles: {},
    };
    const drawNode = jest.fn((drawnNode: typeof node) => {
      expect(drawnNode.drawTitleBox).not.toBe(drawTitleBox);
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node, {} as CanvasRenderingContext2D);

    expect(node.drawTitleBox).toBe(drawTitleBox);
  });

  test('recognizes a UUID-backed subgraph node when the host omits subgraph helpers', () => {
    const drawTitleBox = jest.fn();
    const node = {
      id: 'nested',
      type: '244188e0-95d8-45d9-9beb-adafccf862d6',
      size: [240, 180],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      drawTitleBox,
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
    };
    const drawNode = jest.fn((drawnNode: typeof node) => {
      expect(drawnNode.drawTitleBox).not.toBe(drawTitleBox);
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node, {} as CanvasRenderingContext2D);

    expect(node.drawTitleBox).toBe(drawTitleBox);
  });

  test('recognizes a native subgraph object when the host omits helper methods', () => {
    const drawTitleBox = jest.fn();
    const node = {
      id: 'nested',
      type: 'KSampler',
      size: [240, 180],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      drawTitleBox,
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
      subgraph: {},
    };
    const drawNode = jest.fn((drawnNode: typeof node) => {
      expect(drawnNode.drawTitleBox).not.toBe(drawTitleBox);
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node, {} as CanvasRenderingContext2D);

    expect(node.drawTitleBox).toBe(drawTitleBox);
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

  test('arranges widgets against compact face geometry before restoring editor geometry', () => {
    const widget = { name: 'encode_style', y: 144 };
    const arrangedSizes: Array<[number, number]> = [];
    const node = {
      id: 'inside',
      type: 'PromptEncodeStyle',
      pos: [0, 0],
      size: [360, 180],
      widgets: [widget],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
      arrange: jest.fn(() => {
        arrangedSizes.push([node.size[0], node.size[1]]);
        widget.y = node.size[1] - 20;
      }),
      updateArea: jest.fn(),
    };
    const drawNode = jest.fn(() => {
      expect(node.size).toEqual([260, 38]);
      expect(widget.y).toBe(18);
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node, {} as CanvasRenderingContext2D, {
      presentationWidth: 260,
      presentationHeight: 38,
    });

    expect(arrangedSizes).toEqual([
      [260, 38],
      [360, 180],
    ]);
    expect(node.size).toEqual([360, 180]);
    expect(widget.y).toBe(160);
  });

  test('skips unsafe floor-version arrangement when concrete input mirrors are incomplete', () => {
    const arrange = jest.fn(() => {
      throw new Error('floor Comfy would dereference a missing concrete input');
    });
    const drawNode = jest.fn();
    const node = {
      id: 'inside',
      type: 'PromptEncodeStyle',
      pos: [0, 0],
      size: [360, 180],
      inputs: [{ name: 'prompt' }],
      _concreteInputs: [],
      widgets: [{ name: 'prompt', y: 10 }],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
      arrange,
      updateArea: jest.fn(),
    };

    drawNativeLiteGraphCubeCard({ drawNode }, node as never, {} as CanvasRenderingContext2D);

    expect(drawNode).toHaveBeenCalledTimes(1);
    expect(arrange).not.toHaveBeenCalled();
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
      expect(node.widgets_start_y).toBe(6);
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

  test('does not reserve the hidden-slot gutter twice for a semantic prompt textarea', () => {
    const node = {
      id: 'prompt',
      title: 'Positive prompt',
      type: 'CLIPTextEncode',
      size: [320, 240],
      widgets_start_y: 0,
      widgets: [{ name: 'text', type: 'customtext' }],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      onDrawBackground: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
    };
    const drawNode = jest.fn(() => {
      expect(node.widgets_start_y).toBe(2);
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node, {} as CanvasRenderingContext2D);

    expect(node.widgets_start_y).toBe(0);
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

  test('uses durable widget tooltips when Nodes 1 loses transient button labels', () => {
    const missingLabel: { name: string; label?: string; options: { tooltip: string } } = {
      name: 'simple_syrup_replace_media',
      options: { tooltip: 'Replace masks...' },
    };
    const internalLabel = {
      name: 'simple_syrup_add_media',
      label: 'simple_syrup_add_media',
      options: { tooltip: 'Add masks...' },
    };
    const node = {
      id: 'inside',
      type: 'LoadMaskBatch',
      size: [240, 100],
      widgets: [missingLabel, internalLabel],
      drawSlots: jest.fn(),
      drawCollapsedSlots: jest.fn(),
      title_buttons: [],
      strokeStyles: {},
    };
    const drawNode = jest.fn(() => {
      expect(missingLabel.label).toBe('Replace masks...');
      expect(internalLabel.label).toBe('Add masks...');
    });

    drawNativeLiteGraphCubeCard({ drawNode }, node as never, {} as CanvasRenderingContext2D);

    expect(Object.prototype.hasOwnProperty.call(missingLabel, 'label')).toBe(false);
    expect(internalLabel.label).toBe('simple_syrup_add_media');
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
