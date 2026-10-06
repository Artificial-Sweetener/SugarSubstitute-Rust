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

/** Model only the external checkbox event boundary; native ownership remains real. */
function createCheckbox(documentRef: Document): HTMLInputElement {
  const checkbox = documentRef.createElement('input');
  checkbox.type = 'checkbox';
  return checkbox;
}

test('body checkbox delegates one native edit, keeps headers and disabled fields intact, and restores switches', () => {
  const root = document.createElement('div');
  const row = document.createElement('div');
  row.dataset.scFluentRow = '';
  row.dataset.scFluentWidget = 'WidgetToggleSwitch';
  const native = document.createElement('input');
  native.type = 'checkbox';
  native.setAttribute('aria-label', '<literal> use_tiling');
  const header = document.createElement('header');
  const headerToggle = document.createElement('input');
  headerToggle.type = 'checkbox';
  header.append(headerToggle);
  row.append(native);
  root.append(header, row);
  document.body.append(root);
  const edit = jest.fn();
  native.addEventListener('change', edit);
  const adapter = new NativeToggleRows(createCheckbox);
  adapter.reconcile(root);
  const checkbox = row.querySelector<HTMLInputElement>('[data-sc-boolean-checkbox]');
  if (!checkbox) throw new Error('Missing body checkbox');
  expect(checkbox.getAttribute('aria-label')).toBe('<literal> use_tiling');
  expect(checkbox.nextElementSibling?.textContent).toBe('Enable');
  expect(checkbox.parentElement?.tagName).toBe('LABEL');
  expect(row.querySelector('[data-sc-toggle-state]')).toBeNull();
  expect(header.querySelector('[data-sc-boolean-checkbox]')).toBeNull();
  expect(headerToggle.hasAttribute('data-sc-boolean-native')).toBe(false);
  expect(native.hasAttribute('data-sc-boolean-native')).toBe(true);
  checkbox.click();
  expect(native.checked).toBe(true);
  expect(edit).toHaveBeenCalledTimes(1);
  native.checked = false;
  adapter.reconcile(root);
  expect(checkbox.checked).toBe(false);
  native.disabled = true;
  adapter.reconcile(root);
  expect(checkbox.disabled).toBe(true);
  checkbox.click();
  expect(edit).toHaveBeenCalledTimes(1);
  adapter.clear();
  expect(native.hasAttribute('data-sc-boolean-native')).toBe(false);
  expect(row.querySelector('[data-sc-boolean-checkbox]')).toBeNull();
  expect(native.checked).toBe(false);
});

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
  const adapter = new NativeToggleRows(createCheckbox);
  adapter.reconcile(root);
  const state = row.querySelector<HTMLElement>('[data-sc-boolean-caption]');
  expect(state?.textContent).toBe('Enable');
  text.click();
  expect(input.checked).toBe(true);
  expect(state?.textContent).toBe('Enable');
  expect(change).toHaveBeenCalledTimes(1);
  input.click();
  expect(input.checked).toBe(false);
  expect(state?.textContent).toBe('Enable');
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
  expect(row.querySelector('[data-sc-boolean-caption]')).toBeNull();
  input.disabled = false;
  row.click();
  expect(change).toHaveBeenCalledTimes(3);
  expect(root.querySelector('img')).toBeNull();
});

test('keeps the right-side caption beside one Fluent checkbox through native state and component replacement', async () => {
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
  const adapter = new NativeToggleRows(createCheckbox);
  adapter.reconcile(root);
  const state = group.querySelector<HTMLElement>('[data-sc-boolean-caption]');
  expect(state?.previousElementSibling).toBe(group.querySelector('[data-sc-boolean-checkbox]'));
  expect(state?.parentElement?.nextSibling).toBe(switchRoot);
  expect(state?.textContent).toBe('Enable');
  input.checked = false;
  input.setAttribute('aria-checked', 'false');
  await Promise.resolve();
  expect(state?.textContent).toBe('Enable');
  adapter.reconcile(root);
  expect(state?.textContent).toBe('Enable');
  expect(row.querySelectorAll('[data-sc-boolean-caption]')).toHaveLength(1);
  state?.click();
  expect(input.checked).toBe(true);
  expect(state?.textContent).toBe('Enable');
  const replacement = document.createElement('input');
  replacement.type = 'checkbox';
  input.replaceWith(replacement);
  adapter.reconcile(root);
  expect(state?.textContent).toBe('Enable');
  replacement.click();
  expect(state?.textContent).toBe('Enable');
  switchRoot.remove();
  adapter.reconcile(root);
  expect(row.querySelector('[data-sc-boolean-caption]')).toBeNull();
  group.append(switchRoot);
  adapter.reconcile(root);
  expect(state?.isConnected).toBe(true);
  expect(group.querySelectorAll('[data-sc-boolean-caption]')).toHaveLength(1);
  row.dataset.scFluentWidget = 'WidgetDOM';
  adapter.reconcile(root);
  expect(row.querySelector('[data-sc-boolean-caption]')).toBeNull();
  adapter.clear();
  expect(replacement.checked).toBe(true);
  replacement.checked = false;
  replacement.setAttribute('aria-checked', 'false');
  await Promise.resolve();
  expect(state?.textContent).toBe('Enable');
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
  const adapter = new NativeToggleRows(createCheckbox);
  adapter.reconcile(root);
  expect(row.querySelector('[data-sc-boolean-caption]')).toBeNull();
  expect(caption.textContent).toBe('<custom boolean>');
  expect(caption.children).toHaveLength(0);
  adapter.clear();
});
