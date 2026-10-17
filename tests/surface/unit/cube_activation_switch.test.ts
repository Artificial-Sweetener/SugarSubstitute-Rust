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
import { CubeFaceActivationControl } from '../../../frontend/comfyui/ui/surface/CubeFaceActivationControl.js';
import { publishVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyleBoundary.js';
import { NativeCardTitlebar } from '../../../frontend/comfyui/ui/surface/appearance/NativeCardTitlebar.js';
import { Switch } from '@spectrum-web-components/switch/src/Switch.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  document.documentElement.removeAttribute('data-sc-visual-style');
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
  const view = new CubeFaceActivationControl(document, card, onChange);
  const control = view.root;
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
  view.update(card);
  expect(document.activeElement).toBe(input);
  expect(control.textContent).toBe('Disabled');
  expect(getComputedStyle(thumb).transform).toBe('translateX(0)');
  new CubeFaceActivationControl(document, card, onChange).dispose();
  expect(document.querySelectorAll('#sugarcubes-cube-activation-styles')).toHaveLength(1);
  const unrelated = document.createElement('input');
  unrelated.className = 'p-toggleswitch-input';
  document.body.append(unrelated);
  expect(getComputedStyle(unrelated).opacity).not.toBe('0');
  view.dispose();
});

test('resets native thumb margins when Comfy has loaded its switch stylesheet', () => {
  const hostStyle = document.createElement('style');
  hostStyle.textContent = '.p-toggleswitch-handle { margin-top:-8px; top:50%; left:50%; }';
  document.head.append(hostStyle);
  const view = new CubeFaceActivationControl(
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
  const control = view.root;
  document.body.append(control);
  const thumb = control.querySelector<HTMLElement>('.p-toggleswitch-handle');
  if (!thumb) throw new Error('Missing switch thumb');
  const style = getComputedStyle(thumb);
  expect(style.marginTop).toBe('0px');
  expect(style.top).toBe('3px');
  expect(style.left).toBe('3px');
  expect(style.transform).toBe('translateX(20px)');
  view.dispose();
});

test('Spectrum activation uses Adobe switch behavior and delegates exactly one edit per header or switch gesture', async () => {
  publishVisualStyle(document, 'spectrum');
  const node = { id: 'patch', type: 'MahiroCFG' };
  const card = {
    id: 'patch',
    node,
    label: '<img src=x>',
    enabled: true,
    visible: true,
    showActivationControl: true,
  };
  const onChange = jest.fn();
  const view = new CubeFaceActivationControl(document, card, onChange);
  const surface = document.createElement('div');
  surface.className = 'sugarcubes-cube-face__node-card';
  const header = document.createElement('div');
  const title = document.createElement('span');
  title.dataset.testid = 'node-title';
  title.textContent = card.label;
  header.append(title, view.root);
  surface.append(header);
  document.body.append(surface);
  const control = view.root.querySelector('[data-sc-activation-switch]');
  if (!(control instanceof Switch)) throw new Error('Expected the actual Adobe switch');
  await control.updateComplete;
  expect(control.style.getPropertyValue('--mod-switch-control-width')).toBe('40px');
  expect(control.style.getPropertyValue('--mod-switch-control-height')).toBe('20px');
  expect(control.style.getPropertyValue('--mod-switch-height')).toBe('20px');
  const input = control.shadowRoot?.querySelector('input');
  if (!input) throw new Error('Missing Spectrum switch input');
  expect(view.root.querySelector('.p-toggleswitch')).toBeNull();
  expect(input.getAttribute('role')).toBe('switch');
  expect(input.getAttribute('aria-label')).toBe('Disable <img src=x>');
  expect(view.root.querySelector('img')).toBeNull();
  const titlebar = new NativeCardTitlebar();
  titlebar.reconcile(surface, header, false);
  input.click();
  await control.updateComplete;
  expect(onChange).toHaveBeenCalledTimes(1);
  expect(onChange).toHaveBeenLastCalledWith(node, false);
  header.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await control.updateComplete;
  expect(onChange).toHaveBeenCalledTimes(2);
  expect(onChange).toHaveBeenLastCalledWith(node, true);
  expect(input.getAttribute('aria-checked')).toBe('true');
  control.focus();
  await control.updateComplete;
  view.update({ ...card, enabled: false });
  await control.updateComplete;
  expect(control.shadowRoot?.activeElement).toBe(input);
  expect(view.root.querySelector('[data-sc-activation-switch]')).toBe(control);
  expect(onChange).toHaveBeenCalledTimes(2);
  control.disabled = true;
  await control.updateComplete;
  control.click();
  expect(onChange).toHaveBeenCalledTimes(2);
  titlebar.clear();
  view.dispose();
});

test('provider changes retain the accessory, current state and focus without graph edits; disposal releases the subscription', async () => {
  publishVisualStyle(document, 'fluent');
  const card = {
    id: 'patch',
    node: { id: 'patch', type: 'MahiroCFG' },
    label: 'Patch',
    enabled: true,
    visible: true,
    showActivationControl: true,
  };
  const onChange = jest.fn();
  const view = new CubeFaceActivationControl(document, card, onChange);
  const root = view.root;
  document.body.append(root);
  const native = root.querySelector('input');
  native?.focus();
  publishVisualStyle(document, 'spectrum');
  const spectrum = root.querySelector('[data-sc-activation-switch]');
  if (!(spectrum instanceof Switch)) throw new Error('Expected Adobe switch');
  await spectrum.updateComplete;
  expect(spectrum.style.getPropertyValue('--mod-switch-control-width')).toBe('40px');
  expect(spectrum.style.getPropertyValue('--mod-switch-control-height')).toBe('20px');
  expect(spectrum.checked).toBe(true);
  expect(document.activeElement).toBe(spectrum);
  spectrum.click();
  await spectrum.updateComplete;
  expect(onChange).toHaveBeenCalledTimes(1);
  expect(spectrum.checked).toBe(false);
  publishVisualStyle(document, 'fluent');
  expect(view.root).toBe(root);
  const restored = root.querySelector('input');
  expect(restored?.checked).toBe(false);
  expect(document.activeElement).toBe(restored);
  expect(onChange).toHaveBeenCalledTimes(1);
  view.dispose();
  publishVisualStyle(document, 'spectrum');
  expect(root.querySelector('input')).toBe(restored);
});
