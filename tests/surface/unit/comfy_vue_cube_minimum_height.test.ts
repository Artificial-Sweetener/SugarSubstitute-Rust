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
/** Verify Nodes 2.0 shell geometry is included in the Cube minimum height. */

import { resolveComfyVueCubeMinimumHeight } from '../../../frontend/comfyui/ui/surface/ComfyVueCubeMinimumHeight.js';

describe('resolveComfyVueCubeMinimumHeight', () => {
  test('includes the native editor footer beyond the graph-owned face height', () => {
    const root = document.createElement('div');
    defineGeometry(root, { offsetHeight: 780 });

    expect(
      resolveComfyVueCubeMinimumHeight({
        faceMinimumHeight: 723,
        headerFlowHeight: 49,
        footerFlowHeight: 16,
        nodeHeight: 740,
        nodeRoot: root,
      }),
    ).toBe(748);
  });

  test('remains stable after the graph node reaches the resolved minimum', () => {
    const root = document.createElement('div');
    defineGeometry(root, { offsetHeight: 788 });

    expect(
      resolveComfyVueCubeMinimumHeight({
        faceMinimumHeight: 723,
        headerFlowHeight: 49,
        footerFlowHeight: 16,
        nodeHeight: 748,
        nodeRoot: root,
      }),
    ).toBe(748);
  });

  test('never weakens the renderer-independent face minimum', () => {
    const root = document.createElement('div');
    defineGeometry(root, { offsetHeight: 600 });

    expect(
      resolveComfyVueCubeMinimumHeight({
        faceMinimumHeight: 620,
        headerFlowHeight: 20,
        footerFlowHeight: 16,
        nodeHeight: 560,
        nodeRoot: root,
      }),
    ).toBe(620);
  });
});

/** Define deterministic layout metrics absent from jsdom. */
function defineGeometry(
  element: HTMLElement,
  values: Partial<Record<'offsetHeight', number>>,
): void {
  for (const [name, value] of Object.entries(values)) {
    Object.defineProperty(element, name, { configurable: true, value });
  }
}
