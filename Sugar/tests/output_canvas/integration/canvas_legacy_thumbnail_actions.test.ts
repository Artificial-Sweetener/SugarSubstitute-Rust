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
/** Prove Nodes 1 image overlays preserve native drawing, graph geometry, and callback lifetimes. */
import { jest } from '@jest/globals';
import { ComfyLegacyImageReveal } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyLegacyImageReveal.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';
import { canvasRevealTarget } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasReveal.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

test('routes the hovered batch exactly and restores native drawing without retaining a focused overlay', () => {
  const target = canvasRevealTarget(canvasCatalogFixture().entries[0]!.items[0]!);
  const element = document.createElement('canvas');
  document.body.append(element);
  const draw = jest.fn(() => 'native');
  const node: UnknownRecord = {
    id: '1',
    pos: [100, 100],
    size: [300, 400],
    imageIndex: null,
    imgs: [{ src: '/first.png' }, { src: '/second.png' }],
    imageRects: [
      [0, 50, 100, 100],
      [100, 50, 100, 100],
    ],
    widgets: [{ name: '$$canvas-image-preview', y: 50, computedHeight: 200 }],
    onDrawForeground: draw,
  };
  const source = jest.fn((path: string, url: string) => {
    expect(path).toBe('1');
    return url === '/second.png' ? target : null;
  });
  const reveal = jest.fn();
  const canvas: UnknownRecord = {
    canvas: element,
    ds: { scale: 1, offset: [0, 0] },
    graph_mouse: [220, 180],
  };
  const host = new ComfyLegacyImageReveal(document, () => canvas, source, reveal);
  host.reconcile([{ path: '1', node }]);
  try {
    if (typeof node.onDrawForeground !== 'function') throw new Error('Native drawing hook missing');
    expect(Reflect.apply(node.onDrawForeground, node, [])).toBe('native');
    expect(draw).toHaveBeenCalledTimes(1);
    expect(source).toHaveBeenCalledWith('1', '/second.png', 1);
    const action = document.querySelector<HTMLButtonElement>('[data-sc-image-reveal]')!;
    action.click();
    expect(reveal).toHaveBeenCalledWith(target);
    expect(action.isConnected).toBe(false);
    expect(node.pos).toEqual([100, 100]);
    expect(node.size).toEqual([300, 400]);
    Reflect.apply(node.onDrawForeground, node, []);
    const focused = document.querySelector<HTMLButtonElement>('[data-sc-image-reveal]')!;
    focused.focus();
    host.reconcile([]);
    expect(focused.isConnected).toBe(false);
    expect(node.onDrawForeground).toBe(draw);
  } finally {
    host.dispose();
    element.remove();
  }
});
