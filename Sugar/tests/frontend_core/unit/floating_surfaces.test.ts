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
/** Prove every floating surface escapes layout containment and shares bounded geometry. */
import { test, expect } from '@jest/globals';
import {
  FloatingSurface,
  floatingPosition,
} from '../../../frontend/comfyui/ui/menus/FloatingSurface.js';

test('portals are fixed without feature styles and never become dialog descendants', () => {
  const dialog = document.createElement('section');
  dialog.style.cssText = 'display:flex;contain:layout;transform:translate(120px,335px);';
  const footer = document.createElement('footer');
  footer.textContent = 'Not now';
  dialog.append(footer);
  document.body.append(dialog);
  const before = dialog.childNodes.length;
  const popup = document.createElement('div');
  const surface = new FloatingSurface(popup);
  surface.open({ anchor: { kind: 'pointer', x: 100, y: 50 }, dismiss: () => surface.close() });
  expect(popup.parentElement).toBe(document.body);
  expect(dialog.childNodes.length).toBe(before);
  expect(popup.style.getPropertyValue('position')).toBe('fixed');
  expect(popup.style.getPropertyPriority('position')).toBe('important');
  expect(popup.style.left).toBe('100px');
  expect(popup.style.top).toBe('50px');
  expect(popup.getAttribute('popover')).toBe('manual');
  expect(popup.style.getPropertyValue('--sc-surface')).not.toBe('');
  surface.close();
  expect(popup.isConnected).toBe(false);
  dialog.remove();
});

test('bounds, vertical flipping and submenu directions have one geometry policy', () => {
  const reserved = floatingPosition({ kind: 'pointer', x: 216, y: 100 }, 324, 200, 562, 600, 324);
  expect(reserved).toEqual({ left: 4, top: 100 });
  expect(
    floatingPosition(
      {
        kind: 'submenu',
        bounds: { left: reserved.left, right: reserved.left + 324, top: 100, bottom: 118 },
      },
      192,
      110,
      562,
      600,
    ),
  ).toEqual({ left: 324, top: 100 });
  expect(floatingPosition({ kind: 'pointer', x: 216, y: 100 }, 324, 200, 400, 600, 324)).toEqual({
    left: 4,
    top: 100,
  });
  expect(floatingPosition({ kind: 'pointer', x: 790, y: 590 }, 200, 150, 800, 600)).toEqual({
    left: 596,
    top: 446,
  });
  expect(
    floatingPosition(
      { kind: 'below', bounds: { left: 70, right: 120, top: 500, bottom: 530 } },
      200,
      150,
      800,
      600,
    ),
  ).toEqual({ left: 70, top: 346 });
  expect(
    floatingPosition(
      { kind: 'submenu', bounds: { left: 600, right: 760, top: 100, bottom: 132 } },
      200,
      150,
      800,
      600,
    ),
  ).toEqual({ left: 404, top: 100 });
  expect(floatingPosition({ kind: 'pointer', x: -100, y: -100 }, 1000, 1000, 800, 600)).toEqual({
    left: 4,
    top: 4,
  });
});
