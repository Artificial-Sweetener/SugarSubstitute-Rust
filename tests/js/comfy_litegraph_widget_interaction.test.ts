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
/** Characterize pointer routing through Comfy's native Nodes 1.0 widget path. */

import { jest } from '@jest/globals';
import { ComfyLiteGraphWidgetInteraction } from '../../frontend/comfyui/ui/surface/ComfyLiteGraphWidgetInteraction.js';

describe('ComfyLiteGraphWidgetInteraction', () => {
  test('maps card coordinates and delegates clicks to Comfy processWidgetClick', () => {
    const widget = { name: 'steps', value: 15 };
    const getWidgetOnPos = jest.fn(() => widget);
    const nativeClick = jest.fn();
    const processWidgetClick = jest.fn(
      (_event: unknown, _node: unknown, _widget: unknown, pointer: { onClick?: () => void }) => {
        pointer.onClick = nativeClick;
      },
    );
    const graphMouse: [number, number] = [0, 0];
    const interaction = new ComfyLiteGraphWidgetInteraction({
      graphMouse,
      processWidgetClick,
    });
    const canvas = document.createElement('canvas');
    Object.defineProperty(canvas, 'getBoundingClientRect', {
      value: () => ({ left: 10, top: 20, width: 240, height: 210 }),
    });
    const node = {
      id: 'inside',
      pos: [100, 200],
      size: [240, 180],
      getWidgetOnPos,
    };

    const mount = interaction.attach(canvas, node, () => ({ width: 240, height: 210 }));
    node.size[1] = 900;
    canvas.dispatchEvent(pointer('pointerdown', 130, 125));
    canvas.dispatchEvent(pointer('pointerup', 130, 125));

    expect(getWidgetOnPos).toHaveBeenCalledWith(220, 275);
    expect(processWidgetClick).toHaveBeenCalledWith(
      expect.objectContaining({ canvasX: 220, canvasY: 275 }),
      node,
      widget,
      expect.any(Object),
    );
    expect(nativeClick).toHaveBeenCalledTimes(1);
    mount.dispose();
  });

  test('routes drag and finalization through callbacks installed by Comfy', () => {
    const widget = { name: 'cfg', value: 6 };
    const onDrag = jest.fn();
    const finallyCallback = jest.fn();
    const interaction = new ComfyLiteGraphWidgetInteraction({
      graphMouse: [0, 0],
      processWidgetClick: (_event, _node, _widget, pointer) => {
        pointer.onDrag = onDrag;
        pointer.finally = finallyCallback;
      },
    });
    const canvas = document.createElement('canvas');
    Object.defineProperty(canvas, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 240, height: 210 }),
    });
    const node = {
      id: 'inside',
      pos: [0, 0],
      size: [240, 180],
      getWidgetOnPos: () => widget,
    };

    interaction.attach(canvas, node);
    canvas.dispatchEvent(pointer('pointerdown', 100, 100));
    canvas.dispatchEvent(pointer('pointermove', 130, 100));
    canvas.dispatchEvent(pointer('pointerup', 130, 100));

    expect(onDrag).toHaveBeenCalledTimes(1);
    expect(finallyCallback).toHaveBeenCalledTimes(1);
  });

  test('uses the rendered card width for native button hit testing and restores graph geometry', () => {
    const widget = { name: 'steps', value: 15 };
    const widthsSeenByClick: number[] = [];
    const node = {
      id: 'inside',
      pos: [100, 200],
      size: [320, 180],
      getWidgetOnPos: jest.fn((x: number) =>
        x >= 100 + Number(node.size[0]) - 24 ? widget : null,
      ),
    };
    const interaction = new ComfyLiteGraphWidgetInteraction({
      graphMouse: [0, 0],
      processWidgetClick: (_event, activeNode, _widget, pointer) => {
        pointer.onClick = () => widthsSeenByClick.push(Number(activeNode.size[0]));
      },
    });
    const canvas = document.createElement('canvas');
    Object.defineProperty(canvas, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 240, height: 210 }),
    });

    interaction.attach(canvas, node, () => ({ width: 240, height: 210 }));
    canvas.dispatchEvent(pointer('pointerdown', 230, 105));
    canvas.dispatchEvent(pointer('pointerup', 230, 105));

    expect(node.getWidgetOnPos).toHaveBeenCalledWith(330, 275);
    expect(widthsSeenByClick).toEqual([240]);
    expect(node.size).toEqual([320, 180]);
  });

  test('supplies the pointer-down event required by promoted subgraph widgets', () => {
    const widget = { name: 'steps', value: 28 };
    const interaction = new ComfyLiteGraphWidgetInteraction({
      graphMouse: [0, 0],
      processWidgetClick: (event, _node, _widget, pointer) => {
        expect(pointer.eDown).toBe(event);
        pointer.onClick = () => {
          widget.value += 1;
        };
      },
    });
    const canvas = document.createElement('canvas');
    Object.defineProperty(canvas, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 240, height: 210 }),
    });
    const node = {
      id: 'inside',
      pos: [0, 0],
      size: [240, 180],
      getWidgetOnPos: () => widget,
    };

    interaction.attach(canvas, node);
    const down = pointer('pointerdown', 100, 100);
    canvas.dispatchEvent(down);
    canvas.dispatchEvent(pointer('pointerup', 100, 100));

    expect(widget.value).toBe(29);
  });

  test('routes expanded face controls without changing collapsed editor state', () => {
    const flags = { collapsed: true, pinned: true };
    const inputs = [{ name: 'model' }];
    const outputs = [{ name: 'LATENT' }];
    const widget = { name: 'steps', value: 28 };
    const node = {
      id: 'inside',
      pos: [0, 0],
      size: [240, 180],
      flags,
      inputs,
      outputs,
      widgets_start_y: 0,
      widgets_up: false,
      getWidgetOnPos: jest.fn(() => {
        expect(node.flags).toEqual({ collapsed: false, pinned: true });
        expect(node.inputs).toBe(inputs);
        expect(node.outputs).toBe(outputs);
        expect(node.widgets_start_y).toBe(6);
        expect(node.widgets_up).toBe(true);
        return widget;
      }),
    };
    const interaction = new ComfyLiteGraphWidgetInteraction({
      graphMouse: [0, 0],
      processWidgetClick: (_event, activeNode, _widget, nativePointer) => {
        expect(activeNode.flags).toEqual({ collapsed: false, pinned: true });
        expect(activeNode.inputs).toBe(inputs);
        expect(activeNode.outputs).toBe(outputs);
        expect(activeNode.widgets_start_y).toBe(6);
        expect(activeNode.widgets_up).toBe(true);
        nativePointer.onClick = () => {
          expect(activeNode.flags).toEqual({ collapsed: false, pinned: true });
          expect(activeNode.inputs).toBe(inputs);
          expect(activeNode.outputs).toBe(outputs);
          expect(activeNode.widgets_start_y).toBe(6);
          expect(activeNode.widgets_up).toBe(true);
          widget.value += 1;
        };
      },
    });
    const canvas = document.createElement('canvas');
    Object.defineProperty(canvas, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 240, height: 210 }),
    });

    interaction.attach(canvas, node);
    canvas.dispatchEvent(pointer('pointerdown', 100, 100));
    canvas.dispatchEvent(pointer('pointerup', 100, 100));

    expect(widget.value).toBe(29);
    expect(node.flags).toBe(flags);
    expect(node.flags.collapsed).toBe(true);
    expect(node.inputs).toBe(inputs);
    expect(node.outputs).toBe(outputs);
    expect(node.widgets_start_y).toBe(0);
    expect(node.widgets_up).toBe(false);
  });
});

/** Build one jsdom pointer-shaped event with stable client coordinates. */
function pointer(type: string, clientX: number, clientY: number): Event {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    clientX: { value: clientX },
    clientY: { value: clientY },
    pointerId: { value: 1 },
  });
  return event;
}
