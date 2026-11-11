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
import { describe, expect, jest, test } from '@jest/globals';
import {
  applyComfyNodeSize,
  authoredNodePresentationRect,
  measureNodePresentationRect,
} from '../../../frontend/comfyui/ui/geometry/ComfyNodeGeometry.js';
import { resolveRendererGeometryPolicy } from '../../../frontend/comfyui/ui/geometry/RendererGeometryPolicy.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

describe('Comfy node geometry', () => {
  test('seeds Nodes 2 with authored height and its renderer width floor', () => {
    const setSize = jest.fn<(size: number[]) => void>();
    const computeSize = jest.fn(() => [190, 180]);
    const node: ComfyNode = {
      type: 'KSampler',
      size: [140, 60],
      computeSize,
      setSize,
    };

    expect(
      applyComfyNodeSize(node, [160, 100], resolveRendererGeometryPolicy({ vueNodesMode: true })),
    ).toEqual([225, 100]);
    expect(setSize).toHaveBeenCalledWith([225, 100]);
    expect(computeSize).not.toHaveBeenCalled();
  });

  test('LiteGraph mode applies the exact authored size used by the preview', () => {
    const computeSize = jest.fn(() => [150, 120]);
    const setSize = jest.fn<(size: number[]) => void>();
    const node: ComfyNode = { size: [200, 90], computeSize, setSize };

    expect(applyComfyNodeSize(node, [140, 100], resolveRendererGeometryPolicy({}))).toEqual([
      140, 100,
    ]);
    expect(setSize).toHaveBeenCalledWith([140, 100]);
    expect(computeSize).not.toHaveBeenCalled();
  });

  test('does not mix legacy computeSize into DOM measurement', () => {
    const onResize = jest.fn();
    const node: ComfyNode = {
      type: 'Broken',
      size: [100, 50],
      computeSize: () => {
        throw new Error('measurement failed');
      },
      onResize,
    };

    expect(
      applyComfyNodeSize(node, [140, 60], resolveRendererGeometryPolicy({ vueNodesMode: true })),
    ).toEqual([225, 60]);
    expect(onResize).toHaveBeenCalledWith([225, 60]);
  });

  test('collapsed Nodes 2 retain expanded authored height behind the presentation', () => {
    const node: ComfyNode = { size: [140, 30], flags: { collapsed: true } };

    expect(
      applyComfyNodeSize(node, [140, 30], resolveRendererGeometryPolicy({ vueNodesMode: true })),
    ).toEqual([225, 30]);
  });

  test('collapsed measurement uses the title rectangle while preserving its anchor', () => {
    const node: ComfyNode = {
      pos: [350, 50],
      size: [225, 0],
      renderingSize: [225, 0],
      flags: { collapsed: true },
    };

    expect(measureNodePresentationRect(node)).toEqual({ x: 350, y: 20, w: 225, h: 30 });
    expect(
      authoredNodePresentationRect(
        { x: 350, y: 50, w: 287.4, h: 122 },
        true,
        'Schedule & Encode Prompts',
      ),
    ).toEqual({ x: 350, y: 20, w: 270, h: 30 });
  });

  test('Nodes 2 measurement uses the mounted card instead of the layout-store size', () => {
    const element = document.createElement('article');
    element.dataset.nodeId = '42';
    Object.defineProperties(element, {
      offsetWidth: { value: 320 },
      offsetHeight: { value: 241 },
    });
    document.body.append(element);
    const node: ComfyNode = { id: 42, pos: [10, 20], size: [320, 180] };

    expect(measureNodePresentationRect(node, { renderer: 'vue', document })).toEqual({
      x: 10,
      y: -10,
      w: 320,
      h: 241,
    });
    element.remove();
  });

  test('Nodes 2 collapsed measurement keeps the Comfy title anchor', () => {
    const element = document.createElement('article');
    element.dataset.nodeId = '43';
    Object.defineProperties(element, {
      offsetWidth: { value: 243 },
      offsetHeight: { value: 37 },
    });
    document.body.append(element);
    const node: ComfyNode = {
      id: 43,
      pos: [350, 50],
      size: [287.4, 122],
      flags: { collapsed: true },
    };

    expect(measureNodePresentationRect(node, { renderer: 'vue', document })).toEqual({
      x: 350,
      y: 20,
      w: 243,
      h: 37,
    });
    element.remove();
  });
});
