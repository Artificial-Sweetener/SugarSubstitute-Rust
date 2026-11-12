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
/** Prove graph endpoints use exposed Cube faces and scoped native identities under zoom. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { measureWorkflowMasonryGraph } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryGraphGeometry.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { WorkflowMasonrySection } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('matches repeated internal node IDs to their owning Cube face and ignores unrelated DOM', () => {
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  const layer = document.createElement('div');
  layer.className = 'ph-no-capture';
  const container = document.createElement('div');
  root.append(container, canvas, layer);
  document.body.append(root);
  jest.spyOn(container, 'getBoundingClientRect').mockReturnValue(rectangle(50, 100, 800, 600));
  const sections: WorkflowMasonrySection[] = [];
  const faces: HTMLElement[] = [];
  for (const [index, id] of ['cube-a', 'cube-b'].entries()) {
    const cube = cubeNode(id);
    const native = document.createElement('div');
    native.className = 'lg-node';
    native.dataset.nodeId = id;
    const face = document.createElement('article');
    face.dataset.cubeNodeId = '1';
    face.style.width = '320px';
    native.append(face);
    layer.append(native);
    faces.push(face);
    jest
      .spyOn(face, 'getBoundingClientRect')
      .mockReturnValue(rectangle(70 + index * 200, 180, 160, 100));
    sections.push({
      id,
      label: id,
      cube,
      target: cube,
      cards: [
        {
          id: `cube:${id}:1`,
          node: { id: 1 },
          visible: true,
          enabled: true,
          showActivationControl: false,
          label: 'Seed',
          columnSpan: 1,
        },
      ],
    });
  }
  const outside = document.createElement('div');
  outside.className = 'lg-node';
  outside.dataset.nodeId = 'cube-a';
  document.body.prepend(outside);
  const snapshot = measureWorkflowMasonryGraph(container, sections);
  expect(snapshot.cards.get('cube:cube-a:1')?.element).toBe(faces[0]);
  expect(snapshot.cards.get('cube:cube-b:1')?.element).toBe(faces[1]);
  expect(snapshot.cards.get('cube:cube-b:1')?.rectangle.x).toBe(270);
  expect(snapshot.cards.get('cube:cube-b:1')?.layoutWidth).toBe(320);
  expect(snapshot.cards.get('cube:cube-b:1')?.scale).toBe(0.5);
  expect(snapshot.layers).toEqual([canvas, layer]);
  expect(snapshot.viewport.x).toBe(50);
});

test('measures literal loose-node identities, excludes hidden members, and retains offscreen destinations', () => {
  const root = document.createElement('div');
  document.body.append(root);
  jest.spyOn(root, 'getBoundingClientRect').mockReturnValue(rectangle(0, 0, 800, 600));
  const node = { id: 'a"]body', pos: [20, 40] };
  const native = document.createElement('div');
  native.className = 'lg-node';
  native.dataset.nodeId = node.id;
  root.append(native);
  jest.spyOn(native, 'getBoundingClientRect').mockReturnValue(rectangle(900, 20, 320, 100));
  const section: WorkflowMasonrySection = {
    id: 'loose',
    label: 'Nodes',
    cube: null,
    target: { _nodes: [node] },
    cards: [
      {
        id: 'root:literal',
        node,
        visible: true,
        enabled: true,
        showActivationControl: false,
        label: 'Example',
        columnSpan: 1,
      },
    ],
  };
  const before = JSON.stringify(node);
  const snapshot = measureWorkflowMasonryGraph(root, [section]);
  expect(snapshot.cards.get('root:literal')?.element).toBe(native);
  expect(snapshot.cards.get('root:literal')?.visible).toBe(false);
  expect(JSON.stringify(node)).toBe(before);
  section.cards[0]!.visible = false;
  expect(measureWorkflowMasonryGraph(root, [section]).cards.size).toBe(0);
});

/** Supply browser-coordinate geometry at the JSDOM adapter boundary. */
function rectangle(x: number, y: number, width: number, height: number): DOMRect {
  return {
    x,
    y,
    width,
    height,
    top: y,
    left: x,
    right: x + width,
    bottom: y + height,
    toJSON: () => null,
  };
}

/** Supply a typed Cube whose internal node identity is intentionally reusable. */
function cubeNode(id: string): CubeNode {
  return {
    id,
    type: 'SugarCube',
    pos: [0, 0],
    size: [400, 300],
    properties: {},
    inputs: [],
    outputs: [],
    widgets: [],
    connect() {},
    subgraph: {
      id: `definition-${id}`,
      name: id,
      _nodes: [],
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      addInput() {
        return { connect() {} };
      },
      addOutput() {
        return { connect() {} };
      },
      configure() {},
    },
    isSubgraphNode: () => true,
    serialize: () => ({}),
  };
}
