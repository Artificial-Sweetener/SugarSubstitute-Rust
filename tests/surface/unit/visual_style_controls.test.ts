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
/** Exercise real provider controls through the shared normalized control contracts. */
import { afterEach, expect, test } from '@jest/globals';
import {
  createNumericSlider,
  createButton,
  createBooleanSegments,
} from '../../../frontend/comfyui/ui/surface/appearance/ControlFactory.js';
import { publishVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyleBoundary.js';
import { spectrumRootTokens } from '../../../frontend/comfyui/ui/surface/appearance/SpectrumThemeTokens.js';
import { MenuItem } from '@spectrum-web-components/menu/src/MenuItem.js';
import { applyOwnedTheme } from '../../../frontend/comfyui/ui/surface/appearance/ThemeService.js';

afterEach(() => document.documentElement.removeAttribute('data-sc-visual-style'));
afterEach(() => document.body.replaceChildren());

test.each([true, false])(
  'shares Photoshop neutrals with owned controls and removes them when switching to Fluent, dark=%s',
  (dark) => {
    const surface = document.createElement('div');
    const input = document.createElement('input');
    input.value = '<retained search>';
    surface.append(input);
    document.body.append(surface);
    input.focus();
    applyOwnedTheme(surface, dark, 'spectrum');
    const colors = getComputedStyle(surface);
    expect(colors.getPropertyValue('--sc-photoshop-panel')).toBe(
      `var(--uxp-host-background-color,${dark ? '#323232' : '#f0f0f0'})`,
    );
    expect(colors.getPropertyValue('--sc-photoshop-field')).toBe(dark ? '#2a2a2a' : '#fcfcfc');
    expect(colors.getPropertyValue('--system-textfield-background-color')).toBe(
      'var(--sc-photoshop-field)',
    );
    expect(colors.getPropertyValue('--system-textfield-border-color')).toBe(
      'var(--sc-photoshop-stroke)',
    );
    expect(colors.getPropertyValue('--spectrum-gray-200')).toBe('var(--sc-photoshop-hover)');
    applyOwnedTheme(surface, dark, 'fluent');
    expect(getComputedStyle(surface).getPropertyValue('--sc-photoshop-panel')).toBe('');
    expect(surface.dataset.scTheme).toBe('fluent');
    expect(surface.firstChild).toBe(input);
    expect(document.activeElement).toBe(input);
    expect(input.value).toBe('<retained search>');
    expect(surface.querySelector('retained')).toBeNull();
    applyOwnedTheme(surface, null);
  },
);

test.each([true, false])(
  'replaces browser popover borders on Spectrum menus for dark=%s',
  (dark) => {
    const menu = document.createElement('sp-menu');
    menu.className = 'sc-context-menu';
    menu.setAttribute('popover', 'manual');
    document.body.append(menu);
    applyOwnedTheme(menu, dark, 'spectrum');
    expect(getComputedStyle(menu).borderTopWidth).toBe('1px');
    expect(getComputedStyle(menu).borderTopStyle).toBe('solid');
    expect(menu.dataset.scColorScheme).toBe(dark ? 'dark' : 'light');
    applyOwnedTheme(menu, null);
  },
);

test('retains shared menu focus and radio semantics through the first Adobe render', async () => {
  const row = document.createElement('sc-spectrum-menu-item');
  expect(row).toBeInstanceOf(MenuItem);
  if (!(row instanceof MenuItem)) throw new TypeError('Spectrum menu registration is missing.');
  row.setAttribute('role', 'menuitemradio');
  row.setAttribute('tabindex', '-1');
  row.selected = true;
  row.textContent = '<literal menu label>';
  document.body.append(row);
  row.focus();
  await row.updateComplete;
  await Promise.resolve();
  expect(document.activeElement).toBe(row);
  expect(row.getAttribute('role')).toBe('menuitemradio');
  expect(row.getAttribute('aria-checked')).toBe('true');
  expect(row.querySelector('literal')).toBeNull();
});

test.each(['fluent', 'spectrum'] as const)(
  'normalizes %s slider values without dispatching a native host edit',
  (style) => {
    publishVisualStyle(document, style);
    const slider = createNumericSlider(document);
    slider.synchronize(-1, 1, 0.01, false, 0.37);
    expect(slider.readValue()).toBeCloseTo(0.37);
    expect(slider.root.getAttribute('disabled')).toBeNull();
    slider.synchronize(-1, 1, 0.01, true, -0.19);
    expect(slider.readValue()).toBeCloseTo(-0.19);
    expect(slider.root.tagName.toLowerCase()).toBe(
      style === 'spectrum' ? 'sc-spectrum-slider' : 'fluent-slider',
    );
  },
);

test('creates actual Spectrum buttons and tab segments with stable selected child identities', () => {
  publishVisualStyle(document, 'spectrum');
  const button = createButton(document);
  expect(button.tagName.toLowerCase()).toBe('sp-action-button');
  expect(button.hasAttribute('data-sc-button')).toBe(true);
  const { group, segments, release } = createBooleanSegments(document);
  group.append(...segments);
  group.activeid = segments[1].id;
  expect(group.activeid).toBe(segments[1].id);
  expect(segments.map((segment) => segment.getAttribute('value'))).toEqual(
    segments.map((segment) => segment.id),
  );
  release();
});

test('extracts scoped Adobe tokens without copying global typography or unrelated selectors', () => {
  expect(
    spectrumRootTokens(':root,:host{--a:1;} .spectrum-Typography{color:red}:host,:root{--b:2;}'),
  ).toBe('--a:1;;--b:2;');
  expect(spectrumRootTokens('body{--unrelated:value}')).toBe('');
});
