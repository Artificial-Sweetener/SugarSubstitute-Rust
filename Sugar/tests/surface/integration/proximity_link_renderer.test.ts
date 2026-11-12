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
/** Verify proximity guides use live sockets and request an unbordered native stroke. */

import { jest } from '@jest/globals';

import {
  ProximityLinkRenderer,
  type ProximityRenderCanvas,
} from '../../../frontend/comfyui/ui/overlays/proximity/ProximityLinkRenderer.js';
import type { ProximityMatch } from '../../../frontend/comfyui/ui/overlays/proximity/ProximityModel.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

test('renders an unbordered guide from authoritative animated port positions', () => {
  const graph = {} as ComfyGraph;
  const outputNode: ComfyNode = {
    id: 'output',
    graph,
    pos: [0, 0],
    size: [400, 300],
    outputs: [{ type: 'IMAGE' }],
    getSlotPosition: () => [400, 100],
  };
  const inputNode: ComfyNode = {
    id: 'input',
    graph,
    pos: [450, 0],
    size: [400, 300],
    inputs: [{ type: 'IMAGE' }],
    getSlotPosition: () => [450, 200],
  };
  const renderLink = jest.fn();
  const context = {
    save: jest.fn(),
    restore: jest.fn(),
    setLineDash: jest.fn(),
    lineDashOffset: 0,
    strokeStyle: '',
  } as unknown as CanvasRenderingContext2D;
  const canvas: ProximityRenderCanvas = {
    graph,
    renderLink,
    connections_width: 3,
    default_link_color: '#fff',
  };
  const renderer = new ProximityLinkRenderer({
    getLiteGraph: () => null,
    logger: { debug: jest.fn(), warn: jest.fn() },
  });
  renderer.setPositionSource({
    resolveGraphPosition: (_node, direction) => (direction === 'output' ? [400, 150] : [450, 150]),
  });

  renderer.render([match(outputNode, inputNode)], context, canvas);

  expect(renderLink).toHaveBeenCalledWith(
    context,
    [400, 150],
    [450, 150],
    expect.anything(),
    true,
    false,
    null,
    4,
    3,
    { disabled: false },
  );
});

test('restores native marker and canvas state when the host link painter fails', () => {
  const context = {
    save: jest.fn(),
    restore: jest.fn(),
    setLineDash: jest.fn(),
    lineDashOffset: 0,
    strokeStyle: '',
  } as unknown as CanvasRenderingContext2D;
  const error = new Error('Native link painter failed');
  const warn = jest.fn();
  const canvas: ProximityRenderCanvas = {
    linkMarkerShape: 7,
    render_connections_border: true,
    renderLink: () => {
      throw error;
    },
  };
  const renderer = new ProximityLinkRenderer({
    getLiteGraph: () => null,
    logger: { debug: jest.fn(), warn },
  });

  renderer.render([match({ pos: [0, 0] }, { pos: [50, 0] })], context, canvas);

  expect(context.save).toHaveBeenCalledTimes(1);
  expect(context.restore).toHaveBeenCalledTimes(1);
  expect(canvas.linkMarkerShape).toBe(7);
  expect(canvas.render_connections_border).toBe(true);
  expect(warn).toHaveBeenCalledWith('SugarCubes: proximity renderLink failed', error);
});

/** Build one complete match for the renderer boundary. */
function match(outputNode: ComfyNode, inputNode: ComfyNode): ProximityMatch {
  return {
    outputId: 'output',
    outputSlot: 0,
    outputNode,
    outputCube: 'output-definition',
    outputPos: [400, 100],
    inputId: 'input',
    inputSlot: 0,
    inputNode,
    inputCube: 'input-definition',
    inputName: 'image',
    inputPos: [450, 200],
    originId: 'producer',
    originSlot: 0,
    promptTargets: [{ nodeId: 'consumer', inputSlot: 0, inputName: 'image' }],
    distance: 50,
  };
}
