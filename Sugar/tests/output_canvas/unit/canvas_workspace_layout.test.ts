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
/** Prove editor-width docked/drawer allocation and reversible divider previews without DOM or persistence. */

import { allocateCanvasWorkspace } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasWorkspace.js';
import { CanvasWorkspaceLayout } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasWorkspaceLayout.js';

const DEFAULT_EDITOR = 1096;

test('keeps the default editor width and gives the canvas every new pixel as the window widens', () => {
  const preferences = { visible: true, editorWidth: 0 };
  for (const width of [1500, 1900, 2400]) {
    const allocation = allocateCanvasWorkspace(width, 800, preferences, false, DEFAULT_EDITOR);
    expect(allocation.content).toEqual({ x: 0, y: 0, width: DEFAULT_EDITOR, height: 800 });
    expect(allocation.editorWidth).toBe(DEFAULT_EDITOR);
    expect(allocation.canvas).toEqual({
      x: DEFAULT_EDITOR,
      y: 0,
      width: width - DEFAULT_EDITOR,
      height: 800,
    });
  }
});

test('keeps a stored editor width across window widths and clamps it only for the allocation', () => {
  const stored = { visible: true, editorWidth: 900 };
  expect(allocateCanvasWorkspace(1400, 800, stored, false, DEFAULT_EDITOR).canvas.width).toBe(500);
  expect(allocateCanvasWorkspace(2400, 800, stored, false, DEFAULT_EDITOR).canvas.width).toBe(1500);
  // The canvas keeps its minimum width when the window is too narrow for the editor.
  const narrow = allocateCanvasWorkspace(1100, 800, stored, false, DEFAULT_EDITOR);
  expect(narrow.canvas.width).toBe(320);
  expect(narrow.content.width).toBe(780);
  const tooWide = allocateCanvasWorkspace(
    1400,
    800,
    { visible: true, editorWidth: 5000 },
    false,
    0,
  );
  expect(tooWide.canvas.width).toBe(320);
  expect(tooWide.editorWidth).toBe(1080);
  const tooNarrow = allocateCanvasWorkspace(
    1400,
    800,
    { visible: true, editorWidth: 20 },
    false,
    0,
  );
  expect(tooNarrow.content).toEqual({ x: 0, y: 0, width: 360, height: 800 });
  expect(tooNarrow.canvas.x).toBe(tooNarrow.content.width);
  expect(tooNarrow.canvas.width + tooNarrow.content.width).toBe(1400);
  const hidden = allocateCanvasWorkspace(1400, 800, { ...stored, visible: false }, false, 0);
  expect(hidden.canvas.width).toBe(0);
  expect(hidden.content.width).toBe(1400);
  expect(() =>
    allocateCanvasWorkspace(1400, 800, { visible: true, editorWidth: NaN }, false, 0),
  ).toThrow(RangeError);
  expect(() => allocateCanvasWorkspace(1400, 800, stored, false, -1)).toThrow(RangeError);
  expect(() => new CanvasWorkspaceLayout(Infinity)).toThrow(RangeError);
});

test('keeps narrow drawers explicitly opened, overlays content, and closes on breakpoint entry', () => {
  const layout = new CanvasWorkspaceLayout(DEFAULT_EDITOR);
  layout.bounds(800, 600);
  expect(layout.read().visible).toBe(false);
  layout.drawer(true);
  expect(layout.read().mode).toBe('drawer');
  expect(layout.read().canvas.width).toBe(640);
  expect(layout.read().canvas.x).toBe(160);
  expect(layout.read().content).toEqual({ x: 0, y: 0, width: 800, height: 600 });
  layout.preferences({ visible: true, editorWidth: 300 });
  expect(layout.read().canvas.width).toBe(500);
  expect(layout.read().editorWidth).toBe(300);
  layout.bounds(1400, 800);
  expect(layout.read().visible).toBe(true);
  expect(layout.read().content.width).toBe(360);
  layout.bounds(800, 600);
  expect(layout.read().visible).toBe(false);
  layout.drawer(true);
  layout.preferences({ visible: false, editorWidth: 700 });
  expect(layout.read().visible).toBe(false);
});

test('previews divider moves as editor widths and restores the committed width after cancellation', () => {
  const layout = new CanvasWorkspaceLayout(DEFAULT_EDITOR);
  layout.bounds(1400, 800);
  layout.preferences({ visible: true, editorWidth: 800 });
  expect(layout.read().canvas.width).toBe(600);
  expect(layout.resize(20)).toBe(320);
  expect(layout.read().editorWidth).toBe(1080);
  layout.cancelResize();
  expect(layout.read().canvas.width).toBe(600);
  expect(layout.resize(5000)).toBe(1040);
  expect(layout.read().editorWidth).toBe(360);
  layout.preferences({ visible: true, editorWidth: layout.read().editorWidth });
  expect(layout.read().content.width).toBe(360);
  // A wider window widens the canvas while the committed editor width stays put.
  layout.bounds(2000, 800);
  expect(layout.read().content.width).toBe(360);
  expect(layout.read().canvas.width).toBe(1640);
  layout.bounds(0, 0);
  expect(layout.read().canvas.width).toBe(0);
  expect(layout.read().visible).toBe(false);
  expect(() => layout.bounds(-1, 600)).toThrow(RangeError);
  expect(() => layout.resize(Infinity)).toThrow(RangeError);
});
