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
/** Prove every owned theme boundary shares official brand roles and readable primary controls. */
import { afterEach, expect, test } from '@jest/globals';
import { createAccentPalette } from '../../../frontend/comfyui/ui/surface/appearance/AccentPalette.js';
import { accentThemeCss } from '../../../frontend/comfyui/ui/surface/appearance/AccentThemeStyles.js';
import { fluentAccentColorTokens } from '../../../frontend/comfyui/ui/surface/appearance/FluentThemeRuntime.js';

afterEach(() => document.head.replaceChildren());

test('uses the qfluentwidgets dark primary and button interaction variants', () => {
  const css = accentThemeCss('#0078d4', true);
  expect(css).toContain('--sc-accent-color:#29a2ff');
  expect(css).toContain('--sc-accent-hover:#2592e6');
  expect(css).toContain('--sc-accent-pressed:#2587d1');
});

test.each([true, false])('bridges local themes and native controls for dark=%s', (dark) => {
  const p = createAccentPalette('#ffe066', dark);
  const style = document.createElement('style');
  style.textContent = accentThemeCss('#ffe066', dark);
  document.head.append(style);
  const rules = Array.from(style.sheet?.cssRules ?? []).filter(
    (rule): rule is CSSStyleRule => rule instanceof CSSStyleRule,
  );
  const scope = rules.find((rule) => rule.selectorText?.includes('[data-sc-model-appearance]'))!;
  expect(scope.selectorText).toContain('[data-sc-modal-appearance]');
  expect(scope.selectorText).toContain('[id^="SugarCubes."]');
  expect(rules[0]?.style.getPropertyValue('--sc-accent-color')).toBe(p.indicator);
  expect(scope.style.getPropertyValue('--sc-brand-fill')).toBe('var(--sc-accent-color)');
  expect(scope.style.getPropertyPriority('--sc-brand-fill')).toBe('important');
  expect(rules[0]?.style.getPropertyValue('--sc-accent-foreground')).toBe(p.foreground);
  expect(scope.style.getPropertyValue('--sc-text-on-accent')).toBe('var(--sc-accent-foreground)');
  expect(scope.style.getPropertyValue('--spectrum-accent-color-900')).toBe(
    'var(--sc-accent-color)',
  );
  expect(scope.style.getPropertyValue('--spectrum-focus-indicator-color')).toBe(
    'var(--sc-accent-indicator)',
  );
  expect(scope.style.getPropertyPriority('--spectrum-focus-indicator-color')).toBe('important');
  for (const token of [
    '--p-toggleswitch-checked-background',
    '--p-checkbox-checked-background',
    '--p-slider-range-background',
    '--node-component-outline',
    '--component-node-widget-advanced',
  ])
    expect(scope.style.getPropertyValue(token)).toBe('var(--sc-accent-indicator)');
  const primary = rules.find((rule) => rule.selectorText?.endsWith('[data-variant="primary"]'))!;
  expect(primary.style.color).toBe('var(--sc-accent-foreground)');
  expect(rules[0]?.style.getPropertyValue('--sugarcubes-accent')).toBe('');
});

test.each([true, false])('maps distinct checkbox and switch states for dark=%s', (dark) => {
  const p = createAccentPalette('#0078d4', dark);
  const css = accentThemeCss('#0078d4', dark);
  for (const [token, value] of Object.entries({
    '--sc-accent-checkbox-hover': p.checkbox.hover,
    '--sc-accent-checkbox-pressed': p.checkbox.pressed,
    '--sc-accent-toggle-hover': p.toggle.hover,
    '--sc-accent-toggle-pressed': p.toggle.pressed,
    '--sc-accent-selection': p.selection,
    '--sc-accent-foreground-pressed': p.pressedForeground,
  }))
    expect(css).toContain(`${token}:${value}`);
  expect(css).toContain('--p-toggleswitch-checked-hover-background:var(--sc-accent-toggle-hover)');
  expect(css).toContain('--p-checkbox-checked-hover-background:var(--sc-accent-checkbox-hover)');
  expect(css).toContain('--p-button-primary-active-color:var(--sc-accent-foreground-pressed)');
  expect(css).toContain('--colorCompoundBrandBackgroundHover:var(--sc-accent-indicator)');
  expect(css).toContain('--colorCompoundBrandBackgroundPressed:var(--sc-accent-indicator)');
  expect(css).toContain('--mod-checkbox-emphasized-color-down:var(--sc-accent-checkbox-pressed)');
  expect(css).toContain(
    '--spectrum-switch-background-color-emphasized-selected-down:var(--sc-accent-toggle-pressed)',
  );
  expect(css).toContain('--mod-slider-handle-border-color-down:var(--sc-accent-indicator)');
  expect(css).toContain(`border-bottom-color:${p.buttonBottomBorder}`);

  const tokens = fluentAccentColorTokens('#0078d4', dark);
  expect(tokens.colorBrandBackground).toBe(p.button.rest);
  expect(tokens.colorBrandBackgroundHover).toBe(p.button.hover);
  expect(tokens.colorBrandBackgroundPressed).toBe(p.button.pressed);
  expect(tokens.colorCompoundBrandBackgroundPressed).toBe(p.checkbox.pressed);
  expect(tokens.colorCompoundBrandStrokeHover).toBe(p.checkbox.hover);
  expect(tokens.colorCompoundBrandStrokePressed).toBe(p.checkbox.pressed);
  expect(tokens.colorBrandForeground1).toBe(p.link.rest);
  expect(tokens.colorBrandForegroundLinkPressed).toBe(p.link.pressed);
  expect(tokens.colorNeutralForegroundOnBrand).toBe(p.foreground);
});
