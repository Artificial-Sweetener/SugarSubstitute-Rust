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
import type { CubeResizeEdge } from '../../../frontend/comfyui/ui/cube/geometry/CubeResizeGeometry.js';
import { resizeCubeFrame } from '../../../frontend/comfyui/ui/cube/geometry/CubeResizeGeometry.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { ComfyLiteGraphCubeNodeInteraction } from '../../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeNodeInteraction.js';
import type { ComfyLiteGraphWidgetInteraction } from '../../../frontend/comfyui/ui/surface/ComfyLiteGraphWidgetInteraction.js';
import { computeCubeCanvasLayout } from '../../../frontend/comfyui/ui/surface/CubeCanvasLayout.js';
import { createDefaultCubeSurfaceState } from '../../../frontend/comfyui/ui/surface/CubeSurfaceState.js';
import type { CubePreviewActions } from '../../../frontend/comfyui/ui/surface/CubePreviewActions.js';
import { layoutCubeCanvasPreviewSections } from '../../../frontend/comfyui/ui/surface/CubePreviewSections.js';

const EDGES: CubeResizeEdge[] = ['n', 'ne', 'e', 'se', 's', 'sw', 'w', 'nw'];

describe('ComfyLiteGraphCubeNodeInteraction', () => {
  test('opens Add Cube from the Nodes 1 canvas action at the pointer anchor', () => {
    const node = cubeNode();
    const state = createDefaultCubeSurfaceState();
    state.preview.visible = false;
    const layout = computeCubeCanvasLayout(node, state, 30, ['add-cube']);
    const target = layout.chromeActions['add-cube'];
    if (!target) throw new Error('Missing Add Cube canvas action.');
    const canvasElement = document.createElement('canvas');
    document.body.replaceChildren(canvasElement);
    Object.defineProperty(canvasElement, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 1200, height: 900 }),
    });
    const onAddCube = jest.fn();
    const interaction = new ComfyLiteGraphCubeNodeInteraction({
      canvas: { canvas: canvasElement, convertCanvasToOffset: (point) => point },
      history: {},
      widgetInteraction: inertWidgetInteraction(),
      getItems: () => [{ node, layout }],
      onEdit: jest.fn(),
      onCardActivationChange: jest.fn(),
      chromeActions: { onAddCube },
    });
    const x = target.x + target.width / 2;
    const y = target.y + target.height / 2;

    canvasElement.dispatchEvent(pointer('pointermove', x, y));

    expect(canvasElement.style.cursor).toBe('pointer');

    canvasElement.dispatchEvent(pointer('pointerdown', x, y));

    expect(onAddCube).toHaveBeenCalledWith(
      expect.objectContaining({ instance_id: 'cube-instance' }),
      { left: x, top: y, right: x, bottom: y },
    );

    canvasElement.dispatchEvent(pointer('pointermove', layout.frame.x + 100, layout.frame.y + 100));

    expect(canvasElement.style.cursor).toBe('');
    interaction.dispose();
  });

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
    const onGeometryChange = jest.fn();
    const interaction = new ComfyLiteGraphCubeNodeInteraction({
      canvas: {
        canvas: canvasElement,
        convertCanvasToOffset: (point) => point,
        setDirty: jest.fn(),
      },
      history: { beforeChange, afterChange, setDirtyCanvas: jest.fn() },
      widgetInteraction: inertWidgetInteraction(),
      getItems: () => [{ node, layout }],
      onEdit: jest.fn(),
      onCardActivationChange: jest.fn(),
      onGeometryChange,
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

    expect(onGeometryChange).toHaveBeenCalledWith(node);
    expect(afterChange).not.toHaveBeenCalled();

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
    document.body.replaceChildren(canvasElement);
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
      getItems: () => [{ node, layout }],
      onEdit: openEditor,
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
    document.body.replaceChildren(canvasElement);
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
    document.body.replaceChildren(canvasElement);
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
      getItems: () => [{ node, layout }],
      onEdit: jest.fn(),
      onCardActivationChange: jest.fn(),
    });
    const down = pointer('pointerdown', output.x, output.y);

    canvasElement.dispatchEvent(down);

    expect(down.defaultPrevented).toBe(false);
    expect(beforeChange).not.toHaveBeenCalled();
    interaction.dispose();
  });

  test('routes card activation without renderer-owned visibility controls', () => {
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
    const canvasElement = document.createElement('canvas');
    document.body.replaceChildren(canvasElement);
    Object.defineProperty(canvasElement, 'getBoundingClientRect', {
      value: () => ({ left: 0, top: 0, width: 1200, height: 900 }),
    });
    const onCardActivationChange = jest.fn();
    const interaction = new ComfyLiteGraphCubeNodeInteraction({
      canvas: {
        canvas: canvasElement,
        convertCanvasToOffset: (point) => point,
      },
      history: {},
      widgetInteraction: inertWidgetInteraction(),
      getItems: () => [{ node, layout }],
      onEdit: jest.fn(),
      onCardActivationChange,
    });

    clickCenter(canvasElement, card.activationAction);
    expect(onCardActivationChange).toHaveBeenCalledWith(node, inner, true);
    interaction.dispose();
  });

  test('resizes the persisted preview width without resizing the Cube frame', () => {
    const node = cubeNode();
    const outputSlot = { name: 'image', type: 'IMAGE' };
    node.outputs = [outputSlot];
    node.subgraph.outputs = [outputSlot];
    const state = createDefaultCubeSurfaceState();
    const layout = computeCubeCanvasLayout(node, state, 30);
    if (!layout.previewDivider || !layout.preview) throw new Error('Missing preview divider.');
    const canvasElement = interactionCanvas();
    const beforeChange = jest.fn();
    const afterChange = jest.fn();
    const onPreviewWidthChange = jest.fn();
    const interaction = new ComfyLiteGraphCubeNodeInteraction({
      canvas: { canvas: canvasElement, convertCanvasToOffset: (point) => point },
      history: { beforeChange, afterChange },
      widgetInteraction: inertWidgetInteraction(),
      getItems: () => [{ node, layout }],
      onEdit: jest.fn(),
      onCardActivationChange: jest.fn(),
      onPreviewWidthChange,
    });
    const startX = layout.previewDivider.x + layout.previewDivider.width / 2;
    const startY = layout.previewDivider.y + layout.previewDivider.height / 2;

    canvasElement.dispatchEvent(pointer('pointerdown', startX, startY));
    canvasElement.dispatchEvent(pointer('pointermove', startX - 40, startY));
    canvasElement.dispatchEvent(pointer('pointerup', startX - 40, startY));

    expect(onPreviewWidthChange).toHaveBeenLastCalledWith(node, layout.preview.width + 40);
    expect([...node.size]).toEqual([720, 480]);
    expect(beforeChange).toHaveBeenCalledTimes(1);
    expect(afterChange).toHaveBeenCalledTimes(1);
    interaction.dispose();
  });

  test('routes download and context-menu actions from each canvas preview item', () => {
    const node = cubeNode();
    const outputSlot = { name: 'image', type: 'IMAGE' };
    node.outputs = [outputSlot];
    node.subgraph.outputs = [outputSlot];
    const layout = computeCubeCanvasLayout(node, createDefaultCubeSurfaceState(), 30);
    const firstItem = { key: 'proof-one', url: '/proof-one.png', label: 'image one' };
    const secondItem = { key: 'proof-two', url: '/proof-two.png', label: 'image two' };
    const preview = {
      outputs: [{ id: 'image', label: 'image', items: [firstItem, secondItem] }],
    };
    const section = layoutCubeCanvasPreviewSections(layout.preview, preview)[0];
    if (!section) throw new Error('Missing preview section.');
    const secondLayout = section.items[1];
    if (!secondLayout) throw new Error('Missing second preview item.');
    const previewActions: CubePreviewActions = {
      openContextMenu: jest.fn(),
      download: jest.fn(),
    };
    const canvasElement = interactionCanvas();
    const nativePointerDown = jest.fn();
    canvasElement.addEventListener('pointerdown', nativePointerDown);
    const interaction = new ComfyLiteGraphCubeNodeInteraction({
      canvas: { canvas: canvasElement, convertCanvasToOffset: (point) => point },
      history: {},
      widgetInteraction: inertWidgetInteraction(),
      getItems: () => [{ node, layout, preview }],
      previewActions,
      onEdit: jest.fn(),
      onCardActivationChange: jest.fn(),
    });

    clickCenter(canvasElement, secondLayout.downloadAction);
    const rightPointerEvent = pointer(
      'pointerdown',
      secondLayout.rect.x + secondLayout.rect.width / 2,
      secondLayout.rect.y + secondLayout.rect.height / 2,
      2,
    );
    canvasElement.dispatchEvent(rightPointerEvent);
    const menuEvent = pointer(
      'contextmenu',
      secondLayout.rect.x + secondLayout.rect.width / 2,
      secondLayout.rect.y + secondLayout.rect.height / 2,
      2,
    );
    canvasElement.dispatchEvent(menuEvent);

    expect(previewActions.download).toHaveBeenCalledWith(secondItem);
    expect(rightPointerEvent.defaultPrevented).toBe(true);
    expect(nativePointerDown).not.toHaveBeenCalled();
    expect(previewActions.openContextMenu).toHaveBeenCalledWith(secondItem, menuEvent);
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

/** Build a finite canvas surface with pointer-capture seams. */
function interactionCanvas(): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  document.body.append(canvas);
  Object.defineProperty(canvas, 'getBoundingClientRect', {
    value: () => ({ left: 0, top: 0, width: 1200, height: 900 }),
  });
  Object.defineProperty(canvas, 'setPointerCapture', { value: jest.fn() });
  Object.defineProperty(canvas, 'releasePointerCapture', { value: jest.fn() });
  return canvas;
}

/** Build one jsdom pointer-shaped event with stable graph coordinates. */
function pointer(type: string, clientX: number, clientY: number, button = 0): Event {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    button: { value: button },
    clientX: { value: clientX },
    clientY: { value: clientY },
    pointerId: { value: 1 },
  });
  return event;
}
