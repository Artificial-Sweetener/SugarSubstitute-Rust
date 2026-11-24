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
/** Prove the settings navigation rail sits on the selected item and moves with squash and stretch. */
import { expect, jest, test } from '@jest/globals';
import { NavigationRail } from '../../../frontend/comfyui/ui/settings/workspace/NavigationRail.js';

function item(top: number): HTMLElement {
  const element = document.createElement('button');
  Object.defineProperty(element, 'offsetTop', { value: top });
  Object.defineProperty(element, 'offsetHeight', { value: 40 });
  return element;
}

test('the rail is centred on the selected item and appears without moving the first time', () => {
  const rail = new NavigationRail(document);
  const animate = jest.fn();
  rail.element.animate = animate as unknown as HTMLElement['animate'];
  expect(rail.element.hidden).toBe(true);
  rail.place(item(100));
  expect(rail.element.hidden).toBe(false);
  expect(rail.element.style.transform).toBe('translateY(112px)');
  expect(animate).not.toHaveBeenCalled();
  rail.place(null);
  expect(rail.element.hidden).toBe(true);
});

test('moving down stretches the far end first; moving up stretches the near end first', () => {
  const rail = new NavigationRail(document);
  const animate = jest.fn();
  rail.element.animate = animate as unknown as HTMLElement['animate'];
  rail.place(item(100));
  rail.place(item(188));
  expect(rail.element.style.transform).toBe('translateY(200px)');
  const [down, options] = animate.mock.calls[0] as [Keyframe[], KeyframeAnimationOptions];
  expect(options.duration).toBe(600);
  expect(down.map((frame) => [frame.transform, frame.height, frame.offset])).toEqual([
    ['translateY(112px)', '16px', undefined],
    ['translateY(112px)', '104px', 1 / 3],
    ['translateY(200px)', '16px', undefined],
  ]);
  expect(down[0]?.easing).toBe('cubic-bezier(0.9, 0.1, 1, 0.2)');
  expect(down[1]?.easing).toBe('cubic-bezier(0.1, 0.9, 0.2, 1)');
  rail.place(item(100));
  const [up] = animate.mock.calls[1] as [Keyframe[]];
  expect(up.map((frame) => [frame.transform, frame.height])).toEqual([
    ['translateY(200px)', '16px'],
    ['translateY(112px)', '104px'],
    ['translateY(112px)', '16px'],
  ]);
  rail.place(item(100));
  expect(animate).toHaveBeenCalledTimes(2);
});
