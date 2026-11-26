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
/** The docked console takes its height from the graph: the graph ends at the console's top. */
import { expect, jest, test } from '@jest/globals';
import { ComfyCanvasFrameAllocation } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasFrameAllocation.js';
import { ensureCanvasWorkspaceStyles } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasWorkspaceStyles.js';

/** The installed frame shape, measured as a 1600 x 830 workspace under a 78 px header. */
function frame() {
  const parent = document.createElement('div');
  parent.className = 'comfyui-body';
  const root = document.createElement('div');
  parent.append(root);
  root.id = 'graph-canvas-container';
  const chrome = document.createElement('div');
  const splitter = document.createElement('div');
  splitter.className = 'splitter-overlay-bottom';
  const panel = document.createElement('div');
  panel.className = 'graph-canvas-panel';
  splitter.append(panel);
  chrome.append(splitter);
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  root.append(chrome, canvas);
  const box = { left: 0, top: 78, right: 1600, bottom: 908, width: 1600, height: 830 };
  jest
    .spyOn(parent, 'getBoundingClientRect')
    .mockReturnValue({ ...box, x: box.left, y: box.top, toJSON: () => box } as DOMRect);
  // The panel ends where the splitter's padding begins, as the browser lays it out.
  jest.spyOn(panel, 'getBoundingClientRect').mockImplementation(() => {
    const right = box.right - (Number.parseFloat(splitter.style.paddingRight) || 0);
    const bottom = box.bottom - (Number.parseFloat(splitter.style.paddingBottom) || 0);
    const rect = { left: 0, top: 78, right, bottom, width: right, height: bottom - 78 };
    return { ...rect, x: 0, y: 78, toJSON: () => rect } as DOMRect;
  });
  return { root, splitter, panel };
}

test('a console reservation shortens the graph and the panel its bottom chrome rides on, and 0 gives it back', () => {
  const f = frame();
  const allocation = new ComfyCanvasFrameAllocation(f.root, f.panel);
  allocation.apply(480, 200);
  expect(f.root.style.height).toBe('calc(100% - 200px)');
  expect(f.root.style.width).toBe('calc(100% - 480px)');
  expect(f.splitter.style.paddingBottom).toBe('200px');
  expect(f.panel.getBoundingClientRect().bottom).toBe(708);
  allocation.apply(480, 0);
  expect(f.root.style.height).toBe('');
  expect(f.splitter.style.paddingBottom).toBe('');
  allocation.apply(0, 0);
  expect(f.root.style.cssText).toBe('');
  allocation.dispose();
});

test('a reservation never exceeds the graph, and invalid heights are refused', () => {
  const f = frame();
  const allocation = new ComfyCanvasFrameAllocation(f.root, f.panel);
  allocation.apply(0, 5000);
  expect(f.root.style.height).toBe('calc(100% - 830px)');
  expect(() => allocation.apply(0, -1)).toThrow(RangeError);
  expect(() => allocation.apply(0, Number.NaN)).toThrow(RangeError);
  allocation.dispose();
});

test('ComfyUI’s graph toolbar and minimap keep ComfyUI’s own placement in the reduced panel', () => {
  ensureCanvasWorkspaceStyles(document);
  const css = [...document.head.querySelectorAll('style')]
    .map((style) => style.textContent ?? '')
    .join('');
  expect(css).not.toContain('--sc-output-workspace-reserved');
  expect(css).not.toContain('.minimap-main-container');
});
