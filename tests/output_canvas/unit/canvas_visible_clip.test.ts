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
/** Characterize which ancestor overflow clips reach a positioned native surface. */

import { jest } from '@jest/globals';
import { canvasVisibleClip } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasVisibleClip.js';

function rect(left: number, top: number, width: number, height: number): DOMRect {
  return {
    x: left,
    y: top,
    left,
    top,
    right: left + width,
    bottom: top + height,
    width,
    height,
    toJSON: () => ({}),
  } as DOMRect;
}

/** An overflow-clipping graph root occupying the left half of a 1000x800 window. */
function clippingRoot(): HTMLDivElement {
  const root = document.createElement('div');
  root.style.overflow = 'clip';
  jest.spyOn(root, 'getBoundingClientRect').mockReturnValue(rect(0, 0, 500, 800));
  Object.defineProperty(root, 'offsetWidth', { configurable: true, value: 500 });
  Object.defineProperty(root, 'offsetHeight', { configurable: true, value: 800 });
  Object.defineProperty(root, 'clientWidth', { configurable: true, value: 500 });
  Object.defineProperty(root, 'clientHeight', { configurable: true, value: 800 });
  return root;
}

/** Mount a surface in the right half, outside the root's box, under the given ancestry. */
function measure(build: (root: HTMLDivElement, surface: HTMLDivElement) => void) {
  jest.replaceProperty(window, 'innerWidth', 1000);
  jest.replaceProperty(window, 'innerHeight', 800);
  const root = clippingRoot();
  const surface = document.createElement('div');
  build(root, surface);
  document.body.append(root);
  try {
    return canvasVisibleClip(surface, rect(600, 100, 300, 400));
  } finally {
    root.remove();
    jest.restoreAllMocks();
  }
}

test('a fixed surface escapes the overflow clip of an ancestor that is not its containing block', () => {
  const result = measure((root, surface) => {
    surface.style.position = 'fixed';
    root.append(surface);
  });
  expect(result).toEqual({ clip: { x: 600, y: 100, width: 300, height: 400 }, visible: true });
});

test('a fixed surface is clipped by an ancestor whose transform forms its containing block', () => {
  const result = measure((root, surface) => {
    root.style.transform = 'translateX(0px)';
    surface.style.position = 'fixed';
    root.append(surface);
  });
  expect(result.visible).toBe(true);
  expect(result.clip.width).toBe(0);
});

test('an absolute surface escapes a static clipping ancestor but not a positioned one', () => {
  const insideClippedHolder = measure((root, surface) => {
    const holder = document.createElement('div');
    holder.style.position = 'relative';
    surface.style.position = 'absolute';
    holder.append(surface);
    root.style.position = 'static';
    root.append(holder);
  });
  expect(insideClippedHolder.clip.width).toBe(0);
  const outside = measure((root, surface) => {
    surface.style.position = 'absolute';
    root.append(surface);
  });
  expect(outside.clip).toEqual({ x: 600, y: 100, width: 300, height: 400 });
  const positioned = measure((root, surface) => {
    root.style.position = 'relative';
    surface.style.position = 'absolute';
    root.append(surface);
  });
  expect(positioned.clip.width).toBe(0);
});

test('an in-flow surface stays clipped by its overflow-clipping ancestor', () => {
  const result = measure((root, surface) => {
    root.append(surface);
  });
  expect(result.visible).toBe(true);
  expect(result.clip.width).toBe(0);
});
