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
/** Prove shared native moves preserve editing selection and unrelated focus across browser capabilities. */
import { jest } from '@jest/globals';
import { relocateControl } from '../../../frontend/comfyui/ui/core/ControlPlacement.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

test('fallback relocation retains the focused control, its selection and destination ordering', () => {
  const original = document.createElement('div');
  const target = document.createElement('div');
  const control = document.createElement('input');
  const sibling = document.createElement('span');
  control.value = 'native selection';
  original.append(control);
  target.append(sibling);
  document.body.append(original, target);
  control.focus();
  control.setSelectionRange(2, 9, 'backward');
  relocateControl(control, target, sibling);
  expect(document.activeElement).toBe(control);
  expect([control.selectionStart, control.selectionEnd, control.selectionDirection]).toEqual([
    2,
    9,
    'backward',
  ]);
  expect(target.firstChild).toBe(control);
  expect(control.nextSibling).toBe(sibling);
});

test('connected platform moves retain their receiver and never steal unrelated focus', () => {
  const original = document.createElement('div');
  const target = document.createElement('div');
  const control = document.createElement('input');
  const external = document.createElement('button');
  original.append(control);
  document.body.append(original, target, external);
  external.focus();
  let calls = 0;
  Object.defineProperty(target, 'moveBefore', {
    value(this: HTMLElement, element: HTMLElement, before: Node | null): void {
      expect(this).toBe(target);
      calls++;
      this.insertBefore(element, before);
    },
  });
  relocateControl(control, target);
  expect(calls).toBe(1);
  expect(document.activeElement).toBe(external);
  expect(control.parentElement).toBe(target);
});

test.each([true, false])(
  'native moves restore ancestor focus state only when needed: focused=%s',
  (focused) => {
    const original = document.createElement('div');
    const target = document.createElement('div');
    const control = document.createElement('input');
    control.value = 'native selection';
    original.append(control);
    document.body.append(original, target);
    control.focus();
    control.setSelectionRange(2, 9, 'backward');
    Object.defineProperty(target, 'moveBefore', {
      value(element: HTMLElement, before: Node | null): void {
        target.insertBefore(element, before);
        control.focus();
      },
    });
    jest.spyOn(target, 'matches').mockImplementation((selector) => {
      expect(selector).toBe(':focus-within');
      return focused;
    });
    const blur = jest.spyOn(control, 'blur');
    const focus = jest.spyOn(control, 'focus');
    relocateControl(control, target);
    expect(blur).toHaveBeenCalledTimes(focused ? 0 : 1);
    expect(focus).toHaveBeenCalledTimes(focused ? 1 : 2);
    if (!focused) expect(focus).toHaveBeenLastCalledWith({ preventScroll: true });
    expect(document.activeElement).toBe(control);
    expect([control.selectionStart, control.selectionEnd, control.selectionDirection]).toEqual([
      2,
      9,
      'backward',
    ]);
  },
);
