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
/** Prove a masonry scroll is kept by its Cube and lands there again after the cards reflow. */
import { afterEach, expect, test } from '@jest/globals';
import {
  captureMasonryScroll,
  masonryCubeSections,
  restoreMasonryScroll,
} from '../../../frontend/comfyui/ui/surface/masonry/MasonryCubeSections.js';

afterEach(() => document.body.replaceChildren());

/** A masonry with settled section headers at the given layout positions. */
function masonry(sections: readonly [string, number, number][]): HTMLElement {
  const element = document.createElement('div');
  for (const [section, y, height] of sections) {
    const header = document.createElement('header');
    header.dataset.masonrySection = section;
    header.dataset.masonryY = String(y);
    header.dataset.masonryHeight = String(height);
    element.append(header);
  }
  document.body.append(element);
  return element;
}

test('a scroll is kept by the Cube holding the top of the view', () => {
  const element = masonry([
    ['cube:9', 600, 400],
    ['loose:1', 0, 100],
    ['cube:4', 100, 500],
  ]);
  expect(masonryCubeSections(element).map((section) => section.id)).toEqual(['4', '9']);
  element.scrollTop = 0;
  expect(captureMasonryScroll(element)).toEqual({ cube: null, offset: 0 });
  element.scrollTop = 250;
  expect(captureMasonryScroll(element)).toEqual({ cube: '4', offset: 150 });
  element.scrollTop = 640;
  expect(captureMasonryScroll(element)).toEqual({ cube: '9', offset: 40 });
});

test('a kept scroll lands in the same Cube after the cards reflow, or at the top when it is gone', () => {
  const element = masonry([
    ['cube:4', 100, 300],
    ['cube:9', 420, 400],
  ]);
  restoreMasonryScroll(element, { cube: '9', offset: 40 });
  expect(element.scrollTop).toBe(460);
  restoreMasonryScroll(element, { cube: null, offset: 30 });
  expect(element.scrollTop).toBe(30);
  restoreMasonryScroll(element, { cube: 'gone', offset: 40 });
  expect(element.scrollTop).toBe(0);
  element.scrollTop = 200;
  restoreMasonryScroll(element, null);
  expect(element.scrollTop).toBe(0);
});
