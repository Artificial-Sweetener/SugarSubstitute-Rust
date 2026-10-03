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
/** Prove owned switch rendering and interaction without host switch styles. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  createCubeFaceActivationControl,
  updateCubeFaceActivationControl,
} from '../../../frontend/comfyui/ui/surface/CubeFaceActivationControl.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
});

test('renders an accessible switch without a PrimeVue stylesheet and keeps its focused input', () => {
  const node = { id: 'lora', type: 'LoraLoader' };
  const card = {
    id: 'lora',
    node,
    label: '<img src=x>',
    visible: true,
    enabled: false,
    showActivationControl: true,
  };
  const onChange = jest.fn();
  const control = createCubeFaceActivationControl(document, card, onChange);
  document.body.append(control);
  const input = control.querySelector('input');
  const track = control.querySelector<HTMLElement>('.p-toggleswitch-slider');
  const thumb = control.querySelector<HTMLElement>('.p-toggleswitch-handle');
  const root = control.querySelector<HTMLElement>('.p-toggleswitch');
  if (!input || !track || !thumb || !root) throw new Error('Missing activation switch');
  expect(input.getAttribute('role')).toBe('switch');
  expect(input.getAttribute('aria-label')).toBe('Enable <img src=x>');
  expect(control.querySelector('img')).toBeNull();
  expect(getComputedStyle(input).opacity).toBe('0');
  expect(getComputedStyle(input).position).toBe('absolute');
  expect(getComputedStyle(root).width).toBe('40px');
  expect(getComputedStyle(root).height).toBe('20px');
  expect(getComputedStyle(track).borderRadius).toBe('999px');
  expect(getComputedStyle(thumb).transform).toBe('translateX(0)');
  input.focus();
  input.click();
  expect(onChange).toHaveBeenCalledTimes(1);
  expect(onChange).toHaveBeenCalledWith(node, true);
  expect(control.textContent).toBe('Enabled');
  expect(input.getAttribute('aria-checked')).toBe('true');
  expect(getComputedStyle(thumb).transform).toBe('translateX(20px)');
  updateCubeFaceActivationControl(control, card);
  expect(document.activeElement).toBe(input);
  expect(control.textContent).toBe('Disabled');
  expect(getComputedStyle(thumb).transform).toBe('translateX(0)');
  createCubeFaceActivationControl(document, card, onChange);
  expect(document.querySelectorAll('#sugarcubes-cube-activation-styles')).toHaveLength(1);
  const unrelated = document.createElement('input');
  unrelated.className = 'p-toggleswitch-input';
  document.body.append(unrelated);
  expect(getComputedStyle(unrelated).opacity).not.toBe('0');
});

test('resets native thumb margins when Comfy has loaded its switch stylesheet', () => {
  const hostStyle = document.createElement('style');
  hostStyle.textContent = '.p-toggleswitch-handle { margin-top:-8px; top:50%; left:50%; }';
  document.head.append(hostStyle);
  const control = createCubeFaceActivationControl(
    document,
    {
      id: 'patch',
      node: { id: 'patch', type: 'MahiroCFG' },
      label: 'Mahiro CFG',
      enabled: true,
      visible: true,
      showActivationControl: true,
    },
    jest.fn(),
  );
  document.body.append(control);
  const thumb = control.querySelector<HTMLElement>('.p-toggleswitch-handle');
  if (!thumb) throw new Error('Missing switch thumb');
  const style = getComputedStyle(thumb);
  expect(style.marginTop).toBe('0px');
  expect(style.top).toBe('3px');
  expect(style.left).toBe('3px');
  expect(style.transform).toBe('translateX(20px)');
});
