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
/** Prove inline placement preserves native input identity and reverses every owned change. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeInlineComboSearch } from '../../../frontend/comfyui/ui/surface/appearance/NativeInlineComboSearch.js';
import { fluentAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('places the existing native search over the original field without replacing native listeners', () => {
  jest.spyOn(window, 'requestAnimationFrame').mockReturnValue(1);
  const trigger = document.createElement('button');
  trigger.dataset.testid = 'widget-select-default-trigger';
  trigger.setAttribute('aria-label', 'ESRGAN Model');
  const popup = document.createElement('div');
  const shell = document.createElement('div');
  const input = document.createElement('input');
  input.dataset.testid = 'widget-select-default-search-input';
  input.setAttribute('aria-label', 'Search');
  const nativeInput = jest.fn();
  input.addEventListener('input', nativeInput);
  shell.append(input);
  popup.append(shell);
  document.body.append(trigger, popup);
  const field = document.createElement('div');
  field.setAttribute('data-capture-wheel', 'true');
  trigger.replaceWith(field);
  field.append(trigger);
  jest.spyOn(field, 'getBoundingClientRect').mockReturnValue(rectangle(100, 80, 200, 33));
  Object.defineProperty(field, 'offsetHeight', { value: 33 });
  jest.spyOn(trigger, 'getBoundingClientRect').mockReturnValue(rectangle(100, 88, 180, 17));
  jest.spyOn(popup, 'getBoundingClientRect').mockReturnValue(rectangle(100, 120, 180, 200));
  Object.defineProperty(trigger, 'offsetHeight', { value: 33 });
  const presentation = new NativeInlineComboSearch(document);
  input.focus();
  presentation.present(trigger, popup);
  expect(document.activeElement).toBe(input);
  expect(input.getAttribute('aria-label')).toBe('ESRGAN Model');
  expect(input.style.getPropertyValue('--sc-search-top')).toBe('-40px');
  expect(input.style.getPropertyValue('--sc-search-width')).toBe('180px');
  expect(input.style.getPropertyValue('--sc-search-height')).toBe('33px');
  input.value = '<literal search>';
  input.dispatchEvent(new Event('input'));
  expect(nativeInput).toHaveBeenCalledTimes(1);
  expect(input.value).toBe('<literal search>');
  presentation.clear();
  expect(input.getAttribute('aria-label')).toBe('Search');
  expect(input.hasAttribute('data-sc-inline-search')).toBe(false);
  expect(shell.hasAttribute('data-sc-inline-search-shell')).toBe(false);
  expect(input.style.getPropertyValue('--sc-search-top')).toBe('');
  expect(document.activeElement).toBe(input);
});

/** Supply geometry at the browser boundary omitted by JSDOM. */
function rectangle(x: number, y: number, width: number, height: number): DOMRect {
  return {
    x,
    y,
    width,
    height,
    left: x,
    top: y,
    right: x + width,
    bottom: y + height,
    toJSON: () => null,
  };
}

test('leaves custom comboboxes unchanged and releases detached native inputs', () => {
  jest.spyOn(window, 'requestAnimationFrame').mockReturnValue(1);
  const trigger = document.createElement('button');
  const popup = document.createElement('div');
  const input = document.createElement('input');
  input.dataset.testid = 'widget-select-default-search-input';
  popup.append(input);
  document.body.append(trigger, popup);
  const presentation = new NativeInlineComboSearch(document);
  presentation.present(trigger, popup);
  expect(input.hasAttribute('data-sc-inline-search')).toBe(false);
  trigger.dataset.testid = 'widget-select-default-trigger';
  presentation.present(trigger, popup);
  popup.remove();
  presentation.reconcile();
  expect(input.hasAttribute('data-sc-inline-search')).toBe(false);
  presentation.clear();
});

test('keeps advanced footer rounding at the single card boundary in both palettes', () => {
  for (const dark of [true, false]) {
    const css = fluentAppearanceCss(dark);
    expect(css).toMatch(
      /:has\(\[data-testid="advanced-inputs-button"\]\)[^{]+\{\s*border-radius:0;/,
    );
    expect(css).toMatch(
      /\[data-testid="advanced-inputs-button"\]\s*\{\s*border-radius:0 0 var\(--sc-node-card-radius\) var\(--sc-node-card-radius\);/,
    );
    expect(css).toContain('[data-sc-inline-search-shell] > :not(input) { display:none; }');
  }
});
