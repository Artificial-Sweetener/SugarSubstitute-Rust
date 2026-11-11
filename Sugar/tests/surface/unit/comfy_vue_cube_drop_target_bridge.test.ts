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
/** Verify projected Nodes 2 cards remain Comfy's authoritative drop targets. */

import { jest } from '@jest/globals';
import { ComfyVueCubeDropTargetBridge } from '../../../frontend/comfyui/ui/surface/ComfyVueCubeDropTargetBridge.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

describe('ComfyVueCubeDropTargetBridge', () => {
  test('keeps the internal card selected after the enclosing Cube handles drop', () => {
    const inner: ComfyNode = { id: 'inner', onDragOver: jest.fn(() => true) };
    const outer: ComfyNode = { id: 'cube' };
    const outerRoot = document.createElement('div');
    outerRoot.className = 'lg-node';
    const face = document.createElement('div');
    const card = document.createElement('div');
    card.dataset.cubeNodeId = 'inner';
    const target = document.createElement('button');
    card.append(target);
    face.append(card);
    outerRoot.append(face);
    document.body.replaceChildren(outerRoot);
    let selected: ComfyNode | null = null;
    const outerDragOver = jest.fn();
    outerRoot.addEventListener('dragover', outerDragOver);
    outerRoot.addEventListener('drop', () => {
      selected = outer;
    });
    const bridge = new ComfyVueCubeDropTargetBridge({
      face,
      nodes: [inner],
      setDropTarget: (node) => {
        selected = node;
      },
    });

    target.dispatchEvent(new Event('dragover', { bubbles: true, cancelable: true }));
    target.dispatchEvent(new Event('drop', { bubbles: true, cancelable: true }));

    expect(outerDragOver).not.toHaveBeenCalled();
    expect(selected).toBe(inner);
    bridge.dispose();
  });
});
