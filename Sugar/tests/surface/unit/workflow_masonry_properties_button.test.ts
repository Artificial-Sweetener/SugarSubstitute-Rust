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
/** Prove native Properties controls remain disabled only while masonry is active. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WorkflowMasonryPropertiesButton } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryPropertiesButton.js';

afterEach(() => document.body.replaceChildren());

/** Reproduce the native icon contract without depending on localized button text. */
function button(disabled = false): HTMLButtonElement {
  const result = document.createElement('button');
  const icon = document.createElement('i');
  icon.className = 'icon-[lucide--panel-right]';
  result.append(icon);
  result.disabled = disabled;
  const toolbar = document.createElement('div');
  toolbar.className = 'actionbar-container';
  toolbar.append(result);
  document.body.append(toolbar);
  return result;
}

test('uses native disabled styling and blocks repeated pointer and keyboard activation', () => {
  const native = button();
  const click = jest.fn();
  native.addEventListener('click', click);
  const control = new WorkflowMasonryPropertiesButton(document);
  control.setup();
  for (let index = 0; index < 3; index += 1) {
    control.update(true);
    expect(native.disabled).toBe(true);
    expect(native.getAttribute('aria-disabled')).toBe('true');
    native.click();
    expect(click).toHaveBeenCalledTimes(index);
    control.update(false);
    expect(native.disabled).toBe(false);
    expect(native.hasAttribute('aria-disabled')).toBe(false);
    native.click();
    expect(click).toHaveBeenCalledTimes(index + 1);
  }
  control.dispose();
});

test('handles native relocation and remounts and restores pre-existing host state', async () => {
  const first = button(true);
  first.setAttribute('aria-disabled', 'true');
  const control = new WorkflowMasonryPropertiesButton(document);
  control.setup();
  control.update(true);
  const panel = document.createElement('aside');
  panel.dataset.testid = 'properties-panel';
  document.body.append(panel);
  panel.append(first);
  await Promise.resolve();
  expect(first.disabled).toBe(true);
  first.remove();
  const replacement = button();
  await Promise.resolve();
  expect(replacement.disabled).toBe(true);
  replacement.disabled = false;
  replacement.removeAttribute('aria-disabled');
  await Promise.resolve();
  expect(replacement.disabled).toBe(true);
  control.dispose();
  expect(first.disabled).toBe(true);
  expect(first.getAttribute('aria-disabled')).toBe('true');
  expect(replacement.disabled).toBe(false);
  expect(replacement.hasAttribute('aria-disabled')).toBe(false);
  const later = button();
  await Promise.resolve();
  expect(later.disabled).toBe(false);
});

test('does not disable unrelated controls that happen to use the same icon', () => {
  const unrelated = button();
  document.body.append(unrelated);
  const control = new WorkflowMasonryPropertiesButton(document);
  control.setup();
  control.update(true);
  expect(unrelated.disabled).toBe(false);
  control.dispose();
});

test('disables the relocated left-side Properties affordance and restores it on release', () => {
  const native = button();
  native.querySelector('i')!.className = 'icon-[lucide--panel-left]';
  const control = new WorkflowMasonryPropertiesButton(document);
  control.setup();
  control.update(true);
  expect(native.disabled).toBe(true);
  control.update(false);
  expect(native.disabled).toBe(false);
  control.dispose();
});

test('unrelated popup mounts trigger zero Properties reconciliations', async () => {
  const native = button();
  const control = new WorkflowMasonryPropertiesButton(document);
  control.setup();
  control.update(true);
  const query = jest.spyOn(document, 'querySelectorAll');
  const popup = document.createElement('div');
  const input = document.createElement('input');
  popup.append(input);
  document.body.append(popup);
  input.value = 'search';
  input.dispatchEvent(new Event('input'));
  await Promise.resolve();
  expect(query).not.toHaveBeenCalled();
  expect(native.disabled).toBe(true);
  query.mockRestore();
  control.dispose();
});
