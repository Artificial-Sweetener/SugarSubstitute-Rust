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
import { accentThemeCss } from '../../../frontend/comfyui/ui/surface/appearance/AccentThemeStyles.js';

afterEach(() => document.head.replaceChildren());

test.each([true, false])('bridges local themes and native controls for dark=%s', (dark) => {
  const style = document.createElement('style');
  style.textContent = accentThemeCss('#ffe066', dark);
  document.head.append(style);
  const rules = Array.from(style.sheet?.cssRules ?? []).filter(
    (rule): rule is CSSStyleRule => rule instanceof CSSStyleRule,
  );
  const scope = rules.find((rule) => rule.selectorText?.includes('[data-sc-model-appearance]'))!;
  expect(scope.selectorText).toContain('[data-sc-modal-appearance]');
  expect(scope.selectorText).toContain('[id^="SugarCubes."]');
  expect(scope.style.getPropertyValue('--colorBrandBackground')).toBe('#ffe066');
  expect(scope.style.getPropertyPriority('--colorBrandBackground')).toBe('important');
  expect(scope.style.getPropertyValue('--colorNeutralForegroundOnBrand')).toBe('#000000');
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
