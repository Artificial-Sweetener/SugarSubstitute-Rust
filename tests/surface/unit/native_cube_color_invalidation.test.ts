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
/** Prove only the authoritative native wrapper invalidates Cube color tokens. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { ComfyVueCubeColorScope } from '../../../frontend/comfyui/ui/surface/ComfyVueCubeColorScope.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('ignores relocated cards and popups while updating changed or replaced native wrappers', async () => {
  const root = document.createElement('div');
  const wrapper = document.createElement('div');
  wrapper.dataset.testid = 'node-inner-wrapper';
  wrapper.style.backgroundColor = 'rgb(40, 50, 60)';
  const content = document.createElement('section');
  wrapper.append(content);
  root.append(wrapper);
  document.body.append(root);
  const scope = new ComfyVueCubeColorScope(root);
  scope.mount();
  const computed = jest.spyOn(window, 'getComputedStyle');
  content.append(document.createElement('article'));
  content.firstElementChild?.remove();
  content.append(document.createElement('aside'));
  await Promise.resolve();
  expect(computed).not.toHaveBeenCalled();
  wrapper.style.backgroundColor = 'rgb(70, 80, 90)';
  await Promise.resolve();
  expect(root.style.getPropertyValue('--sugarcubes-cube-card-header')).toBe('rgb(70, 80, 90)');
  computed.mockClear();
  const replacement = wrapper.cloneNode(true) as HTMLElement;
  replacement.style.backgroundColor = 'rgb(10, 20, 30)';
  wrapper.replaceWith(replacement);
  await Promise.resolve();
  expect(computed).not.toHaveBeenCalled();
  expect(root.style.getPropertyValue('--sugarcubes-cube-card-header')).toBe('rgb(10, 20, 30)');
  scope.dispose();
});

test('keeps default Cube colors inherited through palette changes instead of freezing computed values', () => {
  const root = document.createElement('div');
  const wrapper = document.createElement('div');
  wrapper.dataset.testid = 'node-inner-wrapper';
  root.append(wrapper);
  document.body.append(root);
  const inherited = document.createElement('div').style;
  inherited.backgroundColor = 'rgb(43, 43, 43)';
  inherited.setProperty('--component-node-background', '#2b2b2b');
  const computed = jest.spyOn(window, 'getComputedStyle').mockReturnValue(inherited);
  const scope = new ComfyVueCubeColorScope(root);
  scope.mount();
  expect(root.style.getPropertyValue('--sugarcubes-cube-card-header')).toBe('');
  expect(root.style.getPropertyValue('--sugarcubes-cube-card-body')).toBe('');
  expect(root.style.getPropertyValue('--sugarcubes-cube-backdrop')).toBe('');
  expect(computed).not.toHaveBeenCalled();
  scope.dispose();
});
