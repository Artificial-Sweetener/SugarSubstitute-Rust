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
/** Verify focused Nodes 1.0 Cube actions do not replace native node behavior. */

import { jest } from '@jest/globals';
import type { CubeResizeEdge } from '../../frontend/comfyui/ui/cube/geometry/CubeResizeGeometry.js';
import { resizeCubeFrame } from '../../frontend/comfyui/ui/cube/geometry/CubeResizeGeometry.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { ComfyLiteGraphCubeNodeInteraction } from '../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeNodeInteraction.js';
import type { ComfyLiteGraphWidgetInteraction } from '../../frontend/comfyui/ui/surface/ComfyLiteGraphWidgetInteraction.js';
import { computeCubeCanvasLayout } from '../../frontend/comfyui/ui/surface/CubeCanvasLayout.js';
import { computeCubeCanvasCardMenuLayout } from '../../frontend/comfyui/ui/surface/CubeCanvasCardMenuLayout.js';
import { createDefaultCubeSurfaceState } from '../../frontend/comfyui/ui/surface/CubeSurfaceState.js';

const EDGES: CubeResizeEdge[] = ['n', 'ne', 'e', 'se', 's', 'sw', 'w', 'nw'];

describe('ComfyLiteGraphCubeNodeInteraction', () => {
  test.each(EDGES)('resizes the real node from the %s handle', (edge) => {
    const node = cubeNode();
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = false;
    const layout = computeCubeCanvasLayout(node, state, 30);
    const handle = layout.resizeHandles.find((candidate) => candidate.edge === edge);
    if (!handle) throw new Error(`Missing ${edge} resize handle.`);
    const canvasElement = document.createElement('canvas');
    document.body.replaceChildren(canvasElement);
    Object.defineProperty(canvasElement, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 1200, height: 900 }),
    });
    Object.defineProperty(canvasElement, 'setPointerCapture', { value: jest.fn() });
    Object.defineProperty(canvasElement, 'releasePointerCapture', { value: jest.fn() });
    const beforeChange = jest.fn();
    const afterChange = jest.fn();
    const interaction = new ComfyLiteGraphCubeNodeInteraction({
      canvas: {
        canvas: canvasElement,
        convertCanvasToOffset: (point) => point,
        setDirty: jest.fn(),
      },
      history: { beforeChange, afterChange, setDirtyCanvas: jest.fn() },
      widgetInteraction: inertWidgetInteraction(),
      getItems: () => [{ node, layout, cardMenuOpen: false }],
      onEdit: jest.fn(),
      onCardMenuToggle: jest.fn(),
      onCardRevealChange: jest.fn(),
      onCardActivationChange: jest.fn(),
    });
    const start: [number, number] = [
      handle.rect.x + handle.rect.width / 2,
      handle.rect.y + handle.rect.height / 2,
    ];
    const delta: [number, number] = [36, 28];
    const expected = resizeCubeFrame({
      edge,
      startPosition: [100, 140],
      startSize: [720, 480],
      delta,
      minimumSize: [320, 180],
    });

    canvasElement.dispatchEvent(pointer('pointerdown', start[0], start[1]));
    canvasElement.dispatchEvent(pointer('pointermove', start[0] + delta[0], start[1] + delta[1]));
    canvasElement.dispatchEvent(pointer('pointerup', start[0] + delta[0], start[1] + delta[1]));

    expect([...node.pos]).toEqual(expected.position);
    expect([...node.size]).toEqual(expected.size);
    expect(beforeChange).toHaveBeenCalledTimes(1);
    expect(afterChange).toHaveBeenCalledTimes(1);
    interaction.dispose();
  });

  test('leaves ordinary header pointerdown available to native selection and movement', () => {
    const node = cubeNode();
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = false;
    const layout = computeCubeCanvasLayout(node, state, 30);
    const canvasElement = document.createElement('canvas');
    Object.defineProperty(canvasElement, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 1200, height: 900 }),
    });
    const openEditor = jest.fn();
    const interaction = new ComfyLiteGraphCubeNodeInteraction({
      canvas: {
        canvas: canvasElement,
        convertCanvasToOffset: (point) => point,
      },
      history: {},
      widgetInteraction: inertWidgetInteraction(),
      getItems: () => [{ node, layout, cardMenuOpen: false }],
      onEdit: openEditor,
      onCardMenuToggle: jest.fn(),
      onCardRevealChange: jest.fn(),
      onCardActivationChange: jest.fn(),
    });
    const movePoint: [number, number] = [layout.header.x + 180, layout.header.y + 20];
    const down = pointer('pointerdown', movePoint[0], movePoint[1]);

    canvasElement.dispatchEvent(down);

    expect(down.defaultPrevented).toBe(false);
    expect(openEditor).not.toHaveBeenCalled();
    interaction.dispose();
  });

  test('does not clear the native resize cursor while the pointer is over a normal node', () => {
    const canvasElement = document.createElement('canvas');
    canvasElement.style.cursor = 'nwse-resize';
    Object.defineProperty(canvasElement, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 1200, height: 900 }),
    });
    const interaction = new ComfyLiteGraphCubeNodeInteraction({
      canvas: {
        canvas: canvasElement,
        convertCanvasToOffset: (point) => point,
      },
      history: {},
      widgetInteraction: inertWidgetInteraction(),
      getItems: () => [],
      onEdit: jest.fn(),
      onCardMenuToggle: jest.fn(),
      onCardRevealChange: jest.fn(),
      onCardActivationChange: jest.fn(),
    });

    canvasElement.dispatchEvent(pointer('pointermove', 480, 320));

    expect(canvasElement.style.cursor).toBe('nwse-resize');
    interaction.dispose();
  });

  test('leaves boundary output port pointerdown available to native linking', () => {
    const node = cubeNode();
    const outputSlot = { name: 'output.image', type: 'IMAGE' };
    node.outputs = [outputSlot];
    node.subgraph.outputs = [outputSlot];
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = true;
    const layout = computeCubeCanvasLayout(node, state, 30);
    const output = layout.outputs[0];
    if (!output) throw new Error('Missing output boundary slot.');
    const canvasElement = document.createElement('canvas');
    Object.defineProperty(canvasElement, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 1200, height: 900 }),
    });
    const beforeChange = jest.fn();
    const interaction = new ComfyLiteGraphCubeNodeInteraction({
      canvas: {
        canvas: canvasElement,
        convertCanvasToOffset: (point) => point,
      },
      history: { beforeChange },
      widgetInteraction: inertWidgetInteraction(),
      getItems: () => [{ node, layout, cardMenuOpen: false }],
      onEdit: jest.fn(),
      onCardMenuToggle: jest.fn(),
      onCardRevealChange: jest.fn(),
      onCardActivationChange: jest.fn(),
    });
    const down = pointer('pointerdown', output.x, output.y);

    canvasElement.dispatchEvent(down);

    expect(down.defaultPrevented).toBe(false);
    expect(beforeChange).not.toHaveBeenCalled();
    interaction.dispose();
  });

  test('routes card menu and activation hit targets independently', () => {
    const inner: CubeNode['subgraph']['_nodes'][number] = {
      id: 'patch',
      type: 'MahiroCFG',
      mode: 4,
      pos: [0, 0],
      size: [240, 100],
      widgets: [],
      inputs: [{ type: 'MODEL' }],
      outputs: [{ type: 'MODEL' }],
      properties: {},
      connect() {},
    };
    const node = cubeNode([inner]);
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = false;
    state.cards.patch = {
      authoredBypass: true,
      revealed: true,
      enabledOverride: false,
      activeMode: 0,
    };
    const layout = computeCubeCanvasLayout(node, state, 30);
    const card = layout.cards[0];
    if (!card?.activationAction) throw new Error('Missing card activation hit target.');
    const menuItem = computeCubeCanvasCardMenuLayout(layout).items[0];
    if (!menuItem) throw new Error('Missing card menu hit target.');
    const canvasElement = document.createElement('canvas');
    Object.defineProperty(canvasElement, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 1200, height: 900 }),
    });
    const onCardMenuToggle = jest.fn();
    const onCardRevealChange = jest.fn();
    const onCardActivationChange = jest.fn();
    let cardMenuOpen = true;
    const interaction = new ComfyLiteGraphCubeNodeInteraction({
      canvas: {
        canvas: canvasElement,
        convertCanvasToOffset: (point) => point,
      },
      history: {},
      widgetInteraction: inertWidgetInteraction(),
      getItems: () => [{ node, layout, cardMenuOpen }],
      onEdit: jest.fn(),
      onCardMenuToggle,
      onCardRevealChange,
      onCardActivationChange,
    });

    clickCenter(canvasElement, layout.cardMenuAction);
    expect(onCardMenuToggle).toHaveBeenCalledWith(node);

    clickCenter(canvasElement, menuItem.rect);
    expect(onCardRevealChange).toHaveBeenCalledWith(node, inner, false);

    cardMenuOpen = false;
    clickCenter(canvasElement, card.activationAction);
    expect(onCardActivationChange).toHaveBeenCalledWith(node, inner, true);
    interaction.dispose();
  });
});

/** Build one real graph-node-shaped Cube for interaction ownership. */
function cubeNode(innerNodes: CubeNode['subgraph']['_nodes'] = []): CubeNode {
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
      _nodes: innerNodes,
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

/** Dispatch a primary pointer at one graph-space rectangle center. */
function clickCenter(
  canvas: HTMLCanvasElement,
  rect: { x: number; y: number; width: number; height: number },
): void {
  canvas.dispatchEvent(pointer('pointerdown', rect.x + rect.width / 2, rect.y + rect.height / 2));
}

/** Provide the no-widget boundary needed by Cube interaction tests. */
function inertWidgetInteraction(): ComfyLiteGraphWidgetInteraction {
  return {
    begin: () => null,
  } as unknown as ComfyLiteGraphWidgetInteraction;
}

/** Build one jsdom pointer-shaped event with stable graph coordinates. */
function pointer(type: string, clientX: number, clientY: number): Event {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    button: { value: 0 },
    clientX: { value: clientX },
    clientY: { value: clientY },
    pointerId: { value: 1 },
  });
  return event;
}
