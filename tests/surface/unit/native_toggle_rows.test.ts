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
/** Prove native toggle delegation, keyboard access, custom control boundaries, and release. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeToggleRows } from '../../../frontend/comfyui/ui/surface/appearance/NativeToggleRows.js';

afterEach(() => document.body.replaceChildren());

test('clicking a toggle row delegates once and preserves direct switch and custom controls', () => {
  const root = document.createElement('div');
  const row = document.createElement('div');
  row.dataset.scFluentRow = '';
  row.dataset.scFluentWidget = 'WidgetToggleSwitch';
  const text = document.createElement('span');
  text.textContent = '<img src=x> use_tiling';
  const input = document.createElement('input');
  input.type = 'checkbox';
  const custom = document.createElement('button');
  row.append(text, input, custom);
  root.append(row);
  document.body.append(root);
  const change = jest.fn();
  input.addEventListener('change', change);
  const adapter = new NativeToggleRows();
  adapter.reconcile(root);
  const state = row.querySelector<HTMLElement>('[data-sc-toggle-state]');
  expect(state?.textContent).toBe('Disabled');
  text.click();
  expect(input.checked).toBe(true);
  expect(state?.textContent).toBe('Enabled');
  expect(change).toHaveBeenCalledTimes(1);
  input.click();
  expect(input.checked).toBe(false);
  expect(state?.textContent).toBe('Disabled');
  expect(change).toHaveBeenCalledTimes(2);
  custom.click();
  expect(change).toHaveBeenCalledTimes(2);
  row.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true }));
  expect(input.checked).toBe(true);
  input.disabled = true;
  row.click();
  expect(change).toHaveBeenCalledTimes(3);
  adapter.clear();
  expect(row.hasAttribute('tabindex')).toBe(false);
  expect(row.hasAttribute('data-sc-toggle-row')).toBe(false);
  expect(row.querySelector('[data-sc-toggle-state]')).toBeNull();
  input.disabled = false;
  row.click();
  expect(change).toHaveBeenCalledTimes(3);
  expect(root.querySelector('img')).toBeNull();
});

test('keeps one state label beside the native switch through external changes and component replacement', async () => {
  const root = document.createElement('div');
  const row = document.createElement('div');
  row.dataset.scFluentRow = '';
  row.dataset.scFluentWidget = 'WidgetToggleSwitch';
  const group = document.createElement('div');
  const switchRoot = document.createElement('div');
  switchRoot.dataset.pcName = 'toggleswitch';
  const input = document.createElement('input');
  input.type = 'checkbox';
  input.checked = true;
  switchRoot.append(input);
  group.append(switchRoot);
  row.append(group);
  root.append(row);
  document.body.append(root);
  const adapter = new NativeToggleRows();
  adapter.reconcile(root);
  const state = group.querySelector<HTMLElement>('[data-sc-toggle-state]');
  expect(state?.nextSibling).toBe(switchRoot);
  expect(state?.textContent).toBe('Enabled');
  input.checked = false;
  input.setAttribute('aria-checked', 'false');
  await Promise.resolve();
  expect(state?.textContent).toBe('Disabled');
  adapter.reconcile(root);
  expect(state?.textContent).toBe('Disabled');
  expect(row.querySelectorAll('[data-sc-toggle-state]')).toHaveLength(1);
  state?.click();
  expect(input.checked).toBe(true);
  expect(state?.textContent).toBe('Enabled');
  const replacement = document.createElement('input');
  replacement.type = 'checkbox';
  input.replaceWith(replacement);
  adapter.reconcile(root);
  expect(state?.textContent).toBe('Disabled');
  replacement.click();
  expect(state?.textContent).toBe('Enabled');
  switchRoot.remove();
  adapter.reconcile(root);
  expect(row.querySelector('[data-sc-toggle-state]')).toBeNull();
  group.append(switchRoot);
  adapter.reconcile(root);
  expect(state?.isConnected).toBe(true);
  row.dataset.scFluentWidget = 'WidgetDOM';
  adapter.reconcile(root);
  expect(row.querySelector('[data-sc-toggle-state]')).toBeNull();
  adapter.clear();
  expect(replacement.checked).toBe(true);
  replacement.checked = false;
  replacement.setAttribute('aria-checked', 'false');
  await Promise.resolve();
  expect(state?.textContent).toBe('Enabled');
});

test('leaves custom boolean controls and their captions unchanged', () => {
  const root = document.createElement('div');
  const row = document.createElement('div');
  row.dataset.scFluentWidget = 'WidgetDOM';
  const input = document.createElement('input');
  input.type = 'checkbox';
  const caption = document.createElement('span');
  caption.textContent = '<custom boolean>';
  row.append(caption, input);
  root.append(row);
  document.body.append(root);
  const adapter = new NativeToggleRows();
  adapter.reconcile(root);
  expect(row.querySelector('[data-sc-toggle-state]')).toBeNull();
  expect(caption.textContent).toBe('<custom boolean>');
  expect(caption.children).toHaveLength(0);
  adapter.clear();
});
