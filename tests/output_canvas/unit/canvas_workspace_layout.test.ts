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
/** Prove docked/drawer allocation and reversible width previews without DOM or persistence. */

import { allocateCanvasWorkspace } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasWorkspace.js';
import { CanvasWorkspaceLayout } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasWorkspaceLayout.js';

test('allocates forty percent while retaining useful content and clamps persisted user widths', () => {
  const preferences = { visible: true, width: 0 };
  const normal = allocateCanvasWorkspace(1400, 800, preferences, false);
  expect(normal.canvas).toEqual({ x: 840, y: 0, width: 560, height: 800 });
  expect(normal.content).toEqual({ x: 0, y: 0, width: 840, height: 800 });
  expect(allocateCanvasWorkspace(960, 800, { ...preferences, width: 20 }, false).canvas.width).toBe(
    320,
  );
  const wide = allocateCanvasWorkspace(1400, 800, { ...preferences, width: 5000 }, false);
  expect(wide.content).toEqual({ x: 0, y: 0, width: 360, height: 800 });
  expect(wide.canvas.x).toBe(wide.content.width);
  expect(wide.canvas.width + wide.content.width).toBe(1400);
});

test('keeps narrow drawers explicitly opened, overlays content, and closes on breakpoint entry', () => {
  const layout = new CanvasWorkspaceLayout();
  layout.bounds(800, 600);
  expect(layout.read().visible).toBe(false);
  layout.drawer(true);
  expect(layout.read().mode).toBe('drawer');
  expect(layout.read().canvas.width).toBe(640);
  expect(layout.read().canvas.x).toBe(160);
  expect(layout.read().content).toEqual({ x: 0, y: 0, width: 800, height: 600 });
  layout.bounds(1400, 800);
  expect(layout.read().visible).toBe(true);
  layout.bounds(800, 600);
  expect(layout.read().visible).toBe(false);
  layout.drawer(true);
  layout.preferences({ visible: false, width: 700 });
  expect(layout.read().visible).toBe(false);
});

test('previews resize through the authoritative clamp and restores committed width after cancellation', () => {
  const layout = new CanvasWorkspaceLayout();
  layout.bounds(1400, 800);
  layout.preferences({ visible: true, width: 600 });
  expect(layout.resize(20)).toBe(320);
  expect(layout.read().canvas.width).toBe(320);
  layout.cancelResize();
  expect(layout.read().canvas.width).toBe(600);
  expect(layout.resize(5000)).toBe(1040);
  layout.preferences({ visible: true, width: 1040 });
  expect(layout.read().content.width).toBe(360);
  layout.bounds(0, 0);
  expect(layout.read().canvas.width).toBe(0);
  expect(layout.read().visible).toBe(false);
  expect(() => layout.bounds(-1, 600)).toThrow(RangeError);
  expect(() => layout.resize(Infinity)).toThrow(RangeError);
});
