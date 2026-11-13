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
/** Prove floating override intent follows the same nearest-edge model as native Run controls. */
import { expect, test } from '@jest/globals';
import {
  captureOverrideToolbarAnchor,
  resolveOverrideToolbarAnchor,
} from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarFloatingGeometry.js';

const initial = { left: 100, top: 50, width: 1200, height: 800 };
const resized = { left: 300, top: 80, width: 600, height: 400 };
const size = { width: 200, height: 42 };

test.each([
  { edge: 'left', position: { x: 104, y: 250 }, result: { x: 304, y: 180 } },
  { edge: 'right', position: { x: 1096, y: 250 }, result: { x: 696, y: 180 } },
  { edge: 'top', position: { x: 500, y: 54 }, result: { x: 500, y: 84 } },
  { edge: 'bottom', position: { x: 500, y: 804 }, result: { x: 500, y: 434 } },
])(
  'retains $edge distance and proportional alignment through pane and viewport changes',
  ({ edge, position, result }) => {
    const anchor = captureOverrideToolbarAnchor(position, initial, size);
    expect(anchor.edge).toBe(edge);
    expect(anchor.distance).toBe(4);
    expect(resolveOverrideToolbarAnchor(anchor, initial, size)).toEqual(position);
    expect(resolveOverrideToolbarAnchor(anchor, resized, size)).toEqual(result);
  },
);

test('retains right-edge intent when asynchronously mounted controls change the toolbar width', () => {
  const anchor = captureOverrideToolbarAnchor({ x: 1096, y: 250 }, initial, size);
  expect(resolveOverrideToolbarAnchor(anchor, resized, { width: 300, height: 42 })).toEqual({
    x: 596,
    y: 180,
  });
});
