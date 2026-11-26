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
/** Prove a searchable choice reads as one field and suggests what its highlight would pick. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  NativeInlineComboSuggestion,
  suggestionText,
} from '../../../frontend/comfyui/ui/surface/appearance/NativeInlineComboSuggestion.js';
import { NativeInlineComboSearch } from '../../../frontend/comfyui/ui/surface/appearance/NativeInlineComboSearch.js';
import { fieldSurfaceCss } from '../../../frontend/comfyui/ui/surface/appearance/FieldSurfaceStyles.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Build Comfy's search popup: a shell with its input, and the narrowed options. */
function popup(options: readonly string[], highlighted: number | null) {
  const element = document.createElement('div');
  const shell = document.createElement('div');
  const input = document.createElement('input');
  input.dataset.testid = 'widget-select-default-search-input';
  shell.append(input);
  element.append(shell);
  options.forEach((label, index) => {
    const option = document.createElement('div');
    option.setAttribute('role', 'option');
    option.setAttribute('textvalue', label);
    option.textContent = label;
    if (index === highlighted) option.setAttribute('data-highlighted', '');
    element.append(option);
  });
  document.body.append(element);
  return { element, shell, input };
}

test('the ghost shows the current value, completes a prefix and names any other highlight', () => {
  expect(suggestionText('', 'euler', 'karras')).toEqual({ typed: '', rest: 'karras' });
  expect(suggestionText('', null, '')).toBeNull();
  expect(suggestionText('eu', 'euler_ancestral', 'karras')).toEqual({
    typed: 'eu',
    rest: 'ler_ancestral',
  });
  expect(suggestionText('EU', 'euler', 'karras')).toEqual({ typed: 'EU', rest: 'ler' });
  expect(suggestionText('euler', 'euler', 'karras')).toBeNull();
  expect(suggestionText('ler', 'euler', 'karras')).toEqual({ typed: 'ler', rest: '  euler' });
  expect(suggestionText('zz', null, 'karras')).toBeNull();
});

test('the ghost follows typing and the highlight, and Tab or Right Arrow at the end pick it', () => {
  const f = popup(['euler', 'euler_ancestral', 'euler_cfg_pp'], 0);
  const suggestion = new NativeInlineComboSuggestion(f.input, f.element, () => 'karras');
  const ghost = f.shell.querySelector<HTMLElement>('[data-sc-inline-search-ghost]')!;
  expect(ghost.getAttribute('aria-hidden')).toBe('true');
  expect(ghost.textContent).toBe('karras');
  f.input.value = 'eu';
  f.input.setSelectionRange(2, 2);
  suggestion.update();
  expect([ghost.children[0]!.textContent, ghost.children[1]!.textContent]).toEqual(['eu', 'ler']);
  // Comfy moves the highlight on Up and Down; the ghost follows it.
  f.element.querySelector('[data-highlighted]')!.removeAttribute('data-highlighted');
  f.element.querySelectorAll('[role="option"]')[1]!.setAttribute('data-highlighted', '');
  suggestion.update();
  expect(ghost.children[1]!.textContent).toBe('ler_ancestral');
  const keys: string[] = [];
  f.input.addEventListener('keydown', (event) => keys.push(event.key));
  const tab = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
  f.input.dispatchEvent(tab);
  expect(tab.defaultPrevented).toBe(true);
  expect(keys).toEqual(['Enter']);
  f.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  expect(keys).toEqual(['Enter', 'Enter']);
  // Right Arrow inside the text moves the caret as usual.
  f.input.setSelectionRange(1, 1);
  f.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  expect(keys).toEqual(['Enter', 'Enter', 'ArrowRight']);
  // With nothing typed, Tab is left to the page.
  f.input.value = '';
  suggestion.update();
  const untouched = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
  f.input.dispatchEvent(untouched);
  expect(untouched.defaultPrevented).toBe(false);
  suggestion.dispose();
  expect(f.shell.querySelector('[data-sc-inline-search-ghost]')).toBeNull();
});

test('a typed text with no match shows no ghost', () => {
  const f = popup([], null);
  const suggestion = new NativeInlineComboSuggestion(f.input, f.element, () => 'euler');
  f.input.value = 'zz';
  suggestion.update();
  expect(f.shell.querySelector<HTMLElement>('[data-sc-inline-search-ghost]')!.hidden).toBe(true);
  suggestion.dispose();
});

test('the inline search hides the value beneath it and leaves fill, edge and underline to the field', () => {
  jest.spyOn(window, 'requestAnimationFrame').mockReturnValue(1);
  const f = popup(['euler'], 0);
  const field = document.createElement('div');
  field.setAttribute('data-capture-wheel', 'true');
  const trigger = document.createElement('button');
  trigger.dataset.testid = 'widget-select-default-trigger';
  trigger.textContent = 'euler';
  field.append(trigger);
  document.body.append(field);
  const search = new NativeInlineComboSearch(document);
  search.present(trigger, f.element);
  expect(trigger.hasAttribute('data-sc-inline-search-target')).toBe(true);
  expect(f.shell.querySelector('[data-sc-inline-search-ghost]')!.textContent).toBe('euler');
  search.clear();
  expect(trigger.hasAttribute('data-sc-inline-search-target')).toBe(false);
  expect(f.shell.querySelector('[data-sc-inline-search-ghost]')).toBeNull();
  const css = fieldSurfaceCss().replace(/\s+/g, ' ');
  expect(css).toContain('[data-sc-style-popup] [data-sc-inline-search] { background:none; }');
  expect(css).not.toMatch(/inline-search\][^}]*sc-brand-border/);
});
