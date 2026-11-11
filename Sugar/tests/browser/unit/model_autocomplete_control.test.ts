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
/** Verify the authoring-wide repeated-entry model autocomplete contract. */

import { ModelAutocompleteControl } from '../../../frontend/comfyui/ui/controls/ModelAutocompleteControl.js';

test('completes each comma-separated model token without giving up after the first entry', () => {
  document.body.replaceChildren();
  const control = new ModelAutocompleteControl({
    documentRef: document,
    options: ['SDXL', 'Flux .1 D', 'Flux .1 Kontext', 'SD 1.5'],
    value: [],
  });
  document.body.append(control.element);

  for (const [authored, expected] of [
    ['SDX', 'SDXL'],
    ['SDXL, Fl', 'SDXL, Flux .1 D'],
    ['SDXL, Flux .1 D, SD 1', 'SDXL, Flux .1 D, SD 1.5'],
  ] as const) {
    control.input.value = authored;
    control.input.setSelectionRange(authored.length, authored.length);
    control.input.dispatchEvent(new Event('input', { bubbles: true }));
    control.input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    expect(control.input.value).toBe(expected);
  }

  expect(control.values()).toEqual(['SDXL', 'Flux .1 D', 'SD 1.5']);
  expect(control.listbox.hidden).toBe(true);
  expect(control.input.getAttribute('aria-expanded')).toBe('false');
  control.dispose();
});

test('renders model names literally and gives every suggestion an accessible identity', () => {
  document.body.replaceChildren();
  const control = new ModelAutocompleteControl({
    documentRef: document,
    options: ['Flux .1 D', 'Flux <img src=x onerror=alert(1)>'],
    value: 'Fl',
  });
  document.body.append(control.element);
  control.input.setSelectionRange(2, 2);
  control.input.dispatchEvent(new Event('input', { bubbles: true }));

  const suggestions = Array.from(
    control.listbox.querySelectorAll<HTMLButtonElement>('[role="option"]'),
  );
  expect(suggestions.map((suggestion) => suggestion.id)).toEqual([
    `${control.listbox.id}-option-0`,
    `${control.listbox.id}-option-1`,
  ]);
  expect(control.listbox.textContent).toContain('Flux <img src=x onerror=alert(1)>');
  expect(control.listbox.querySelector('img')).toBeNull();
  control.dispose();
});

test('portals suggestions beyond clipping ancestors and aligns them to the input', () => {
  document.body.replaceChildren();
  const clippingCard = document.createElement('aside');
  clippingCard.style.overflow = 'hidden';
  const control = new ModelAutocompleteControl({
    documentRef: document,
    options: ['SDXL', 'SDXL Lightning'],
    value: 'SDX',
  });
  clippingCard.append(control.element);
  document.body.append(clippingCard);
  Object.defineProperty(control.input, 'getBoundingClientRect', {
    value: () => ({
      bottom: 134,
      height: 34,
      left: 40,
      right: 320,
      top: 100,
      width: 280,
      x: 40,
      y: 100,
      toJSON: () => ({}),
    }),
  });

  control.input.setSelectionRange(3, 3);
  control.input.dispatchEvent(new Event('input', { bubbles: true }));

  expect(control.listbox.parentElement).toBe(document.body);
  expect(clippingCard.contains(control.listbox)).toBe(false);
  expect(getComputedStyle(control.listbox).position).toBe('fixed');
  expect(control.listbox.style.left).toBe('40px');
  expect(control.listbox.style.top).toBe('138px');
  expect(control.listbox.style.width).toBe('280px');
  control.dispose();
  expect(control.listbox.isConnected).toBe(false);
});
