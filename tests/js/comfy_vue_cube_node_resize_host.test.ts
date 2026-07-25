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
/** Verify Nodes 2.0 edge gestures supplement native corner resizing. */

import { jest } from '@jest/globals';
import {
  resizeCubeFrame,
  type CubeResizeEdge,
} from '../../frontend/comfyui/ui/cube/geometry/CubeResizeGeometry.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { ComfyVueCubeNodeResizeHost } from '../../frontend/comfyui/ui/surface/ComfyVueCubeNodeResizeHost.js';

const EDGES = ['n', 'e', 's', 'w'] as const satisfies readonly CubeResizeEdge[];

describe('ComfyVueCubeNodeResizeHost', () => {
  test.each(EDGES)('resizes the real node from the %s edge at active graph scale', (edge) => {
    const node = cubeNode();
    const root = document.createElement('div');
    document.body.replaceChildren(root);
    const beforeChange = jest.fn();
    const afterChange = jest.fn();
    const setDirtyCanvas = jest.fn();
    const host = new ComfyVueCubeNodeResizeHost({
      root,
      node,
      history: { beforeChange, afterChange, setDirtyCanvas },
      getScale: () => 2,
    });
    const handle = root.querySelector<HTMLElement>(`[data-sugarcube-edge-resize="${edge}"]`);
    if (!handle) throw new Error(`Missing ${edge} edge handle.`);
    const graphDelta: [number, number] = [36, 28];
    const expected = resizeCubeFrame({
      edge,
      startPosition: [100, 140],
      startSize: [720, 480],
      delta: graphDelta,
      minimumSize: [320, 180],
    });

    handle.dispatchEvent(pointer('pointerdown', 200, 240));
    document.dispatchEvent(
      pointer('pointermove', 200 + graphDelta[0] * 2, 240 + graphDelta[1] * 2),
    );
    document.dispatchEvent(pointer('pointerup', 200 + graphDelta[0] * 2, 240 + graphDelta[1] * 2));

    expect([...node.pos]).toEqual(expected.position);
    expect([...node.size]).toEqual(expected.size);
    expect(beforeChange).toHaveBeenCalledTimes(1);
    expect(afterChange).toHaveBeenCalledTimes(1);
    expect(setDirtyCanvas).toHaveBeenCalled();
    host.dispose();
    expect(root.querySelector('[data-sugarcube-edge-resize]')).toBeNull();
  });

  test('resizes from mouse-only host input without starting native node movement', () => {
    const node = cubeNode();
    const root = document.createElement('div');
    document.body.replaceChildren(root);
    const beforeChange = jest.fn();
    const afterChange = jest.fn();
    const host = new ComfyVueCubeNodeResizeHost({
      root,
      node,
      history: { beforeChange, afterChange },
      getScale: () => 1,
    });
    const handle = root.querySelector<HTMLElement>('[data-sugarcube-edge-resize="e"]');
    if (!handle) throw new Error('Missing right-edge handle.');

    const down = mouse('mousedown', 820, 300);
    const move = mouse('mousemove', 900, 300);
    const up = mouse('mouseup', 900, 300);
    handle.dispatchEvent(down);
    document.dispatchEvent(move);
    document.dispatchEvent(up);

    expect([...node.pos]).toEqual([100, 140]);
    expect([...node.size]).toEqual([800, 480]);
    expect(down.defaultPrevented).toBe(true);
    expect(move.defaultPrevented).toBe(true);
    expect(up.defaultPrevented).toBe(true);
    expect(beforeChange).toHaveBeenCalledTimes(1);
    expect(afterChange).toHaveBeenCalledTimes(1);
    host.dispose();
  });

  test('clamps edge resizing to the current content-derived minimum height', () => {
    const node = cubeNode();
    const root = document.createElement('div');
    document.body.replaceChildren(root);
    const host = new ComfyVueCubeNodeResizeHost({
      root,
      node,
      history: {},
      getScale: () => 1,
    });
    host.setMinimumHeight(420);
    const handle = root.querySelector<HTMLElement>('[data-sugarcube-edge-resize="n"]');
    if (!handle) throw new Error('Missing top-edge handle.');

    handle.dispatchEvent(pointer('pointerdown', 200, 240));
    document.dispatchEvent(pointer('pointermove', 200, 500));
    document.dispatchEvent(pointer('pointerup', 200, 500));

    expect([...node.pos]).toEqual([100, 200]);
    expect([...node.size]).toEqual([720, 420]);
    host.dispose();
  });
});

/** Build one real Cube node whose graph geometry is authoritative. */
function cubeNode(): CubeNode {
  const subgraph = {
    id: 'cube-definition',
    name: 'Example Cube',
    _nodes: [],
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
  };
  return {
    id: 'cube-node',
    type: subgraph.id,
    title: subgraph.name,
    pos: [100, 140],
    size: [720, 480],
    properties: {},
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
    setSize(size) {
      this.size[0] = size[0] ?? 0;
      this.size[1] = size[1] ?? 0;
    },
  };
}

/** Build one jsdom pointer-shaped event with stable viewport coordinates. */
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

/** Build one mouse-only event for automation and legacy host input. */
function mouse(type: string, clientX: number, clientY: number): MouseEvent {
  return new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    button: 0,
    buttons: type === 'mouseup' ? 0 : 1,
    clientX,
    clientY,
  });
}
