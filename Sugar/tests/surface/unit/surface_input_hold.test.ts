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
/** Prove a held surface refuses input without being inert and is restored exactly. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  SURFACE_INPUT_HELD_ATTRIBUTE,
  SurfaceInputHold,
} from '../../../frontend/comfyui/ui/surface/motion/SurfaceInputHold.js';

afterEach(() => {
  document.body.replaceChildren();
});

function fixture(): { surface: HTMLElement; button: HTMLButtonElement } {
  const surface = document.createElement('section');
  const button = document.createElement('button');
  surface.append(button);
  document.body.append(surface);
  return { surface, button };
}

test('refuses pointer input and focus inside a held surface without making it inert', () => {
  const { surface, button } = fixture();
  const clicked = jest.fn();
  button.addEventListener('click', clicked);
  button.focus();
  const hold = new SurfaceInputHold();
  hold.hold(surface);
  expect(hold.held).toBe(true);
  expect(surface.inert).toBeUndefined();
  expect(surface.hasAttribute(SURFACE_INPUT_HELD_ATTRIBUTE)).toBe(true);
  expect(surface.getAttribute('style')).toBeNull();
  expect(document.activeElement).not.toBe(button);
  button.click();
  const wheel = new Event('wheel', { bubbles: true, cancelable: true });
  button.dispatchEvent(wheel);
  expect(clicked).not.toHaveBeenCalled();
  expect(wheel.defaultPrevented).toBe(true);
  button.focus();
  expect(document.activeElement).not.toBe(button);
});

test('gives the surface back its input and leaves input outside it alone', () => {
  const { surface, button } = fixture();
  const outside = document.createElement('button');
  document.body.append(outside);
  const outsideClicked = jest.fn();
  outside.addEventListener('click', outsideClicked);
  const clicked = jest.fn();
  button.addEventListener('click', clicked);
  const hold = new SurfaceInputHold();
  hold.hold(surface);
  outside.click();
  outside.focus();
  expect(outsideClicked).toHaveBeenCalledTimes(1);
  expect(document.activeElement).toBe(outside);
  hold.release();
  expect(hold.held).toBe(false);
  expect(surface.hasAttribute(SURFACE_INPUT_HELD_ATTRIBUTE)).toBe(false);
  button.click();
  button.focus();
  expect(clicked).toHaveBeenCalledTimes(1);
  expect(document.activeElement).toBe(button);
});

test('a listener the surface registered first in its own capture phase hears nothing', () => {
  const { surface, button } = fixture();
  const heard = jest.fn();
  surface.addEventListener('pointerdown', heard, true);
  const hold = new SurfaceInputHold();
  hold.hold(surface);
  button.dispatchEvent(new Event('pointerdown', { bubbles: true, cancelable: true }));
  expect(heard).not.toHaveBeenCalled();
});
