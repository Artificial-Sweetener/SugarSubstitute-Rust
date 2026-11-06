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
/** Verify palette reads precede writes and remain shared across mounted model fields. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { ModelPickerAppearance } from '../../../frontend/comfyui/ui/model_library/ModelPickerAppearance.js';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';
import { VISUAL_STYLE_SETTING } from '../../../frontend/comfyui/ui/surface/appearance/VisualStylePreference.js';
import type { VisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyle.js';
import { accentThemeCss } from '../../../frontend/comfyui/ui/surface/appearance/AccentThemeStyles.js';
import { createAccentPalette } from '../../../frontend/comfyui/ui/surface/appearance/AccentPalette.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
  document.documentElement.removeAttribute('style');
});

test.each([true, false])('model actions use accent press roles for dark=%s', (dark) => {
  const palette = createAccentPalette('#0078d4', dark);
  const accent = document.createElement('style');
  const picker = document.createElement('style');
  accent.textContent = accentThemeCss('#0078d4', dark);
  picker.textContent = modelPickerCss;
  document.head.append(accent, picker);
  try {
    const active = Array.from(picker.sheet?.cssRules ?? []).find(
      (rule): rule is CSSStyleRule =>
        rule instanceof CSSStyleRule &&
        rule.selectorText === '.sc-model-find:not(:disabled):active',
    );
    expect(active?.style.background).toBe('var(--sc-brand-fill-pressed)');
    expect(active?.style.color).toBe('var(--sc-accent-foreground-pressed)');
    const root = accent.sheet?.cssRules[0];
    if (!(root instanceof CSSStyleRule)) throw new Error('Missing accent root rule');
    expect(root.style.getPropertyValue('--sc-accent-pressed')).toBe(palette.button.pressed);
    expect(root.style.getPropertyValue('--sc-accent-foreground-pressed')).toBe(
      palette.pressedForeground,
    );
    expect(root.style.getPropertyValue('--sc-accent-foreground')).toBe(palette.foreground);
  } finally {
    accent.remove();
    picker.remove();
  }
});

test('keeps thumbnail-wash captions white while ordinary native fields inherit the host foreground', () => {
  expect(modelPickerCss).toContain('height:var(--sc-control-height,33px)');
  expect(modelPickerCss).toMatch(
    /\[data-sc-model-appearance\]:not\(\[data-sc-model-appearance="native"\]\) \.sc-model-field:has\(\.sc-model-banner\)[^{]*\{[^}]*color:\s*#fff;/,
  );
  expect(modelPickerCss).toMatch(/\.sc-model-transfer-caption\s*\{[^}]*color:\s*#fff;/);
  expect(modelPickerCss).toMatch(
    /\[data-sc-model-appearance="native"\] \.sc-model-field\s*\{[^}]*color:var\(--component-node-foreground\)/,
  );
});

test('keeps transfer text readable on a stable backdrop even without a thumbnail', () => {
  const style = document.createElement('style');
  style.textContent = modelPickerCss;
  document.head.append(style);
  const presentation = document.createElement('div');
  presentation.className = 'sc-model-transfer-presentation';
  document.body.append(presentation);
  expect(getComputedStyle(presentation).backgroundColor).toBe('rgb(32, 32, 36)');
  style.remove();
});

test('shares the active palette across new fields and refreshes it after a host theme change', async () => {
  const readStyle = jest.spyOn(window, 'getComputedStyle');
  const appearance = new ModelPickerAppearance(document, {
    ui: { settings: { getSettingValue: () => true } },
  });
  appearance.activate();
  const paletteReads = () =>
    readStyle.mock.calls.filter(([root]) => root === document.documentElement).length;
  expect(paletteReads()).toBe(1);
  const fields = [document.createElement('div'), document.createElement('div')];
  document.body.append(...fields);
  for (const field of fields) appearance.track(field);
  expect(paletteReads()).toBe(1);
  expect(fields.every((field) => field.dataset.scModelAppearance === 'fluent')).toBe(true);
  document.documentElement.style.colorScheme = 'light';
  await Promise.resolve();
  expect(paletteReads()).toBe(2);
  appearance.release(fields[0]!);
  expect(fields[0]!.dataset.scModelAppearance).toBeUndefined();
  appearance.dispose();
  expect(fields[1]!.dataset.scModelAppearance).toBeUndefined();
});

test('Spectrum picker skin follows provider changes without replacing the active field or emitting a value edit', async () => {
  let style: VisualStyle = 'spectrum';
  const listeners = new Map<string, EventListener>();
  const appearance = new ModelPickerAppearance(document, {
    ui: {
      settings: {
        getSettingValue: (id: string) => (id === VISUAL_STYLE_SETTING ? style : undefined),
        addEventListener: (name: string, listener: EventListener) => listeners.set(name, listener),
        removeEventListener: (name: string) => listeners.delete(name),
      },
    },
  });
  const mount = document.createElement('div');
  const field = document.createElement('div');
  field.className = 'sc-model-field';
  const input = document.createElement('input');
  input.value = '<model name>';
  const edit = jest.fn();
  input.addEventListener('input', edit);
  field.append(input);
  mount.append(field);
  document.body.append(mount);
  appearance.activate();
  appearance.track(mount);
  input.focus();
  input.setSelectionRange(2, 5);
  expect(mount.dataset.scModelAppearance).toBe('spectrum');
  expect(getComputedStyle(mount).getPropertyValue('--sc-field-underline-display')).toBe('none');
  style = 'fluent';
  const eventName = `${VISUAL_STYLE_SETTING}.change`;
  listeners.get(eventName)?.(new Event(eventName));
  await Promise.resolve();
  expect(mount.dataset.scModelAppearance).toBe('fluent');
  expect(mount.style.getPropertyValue('--sc-field-underline-display')).toBe('block');
  expect(mount.querySelector('input')).toBe(input);
  expect(document.activeElement).toBe(input);
  expect(input.value).toBe('<model name>');
  expect([input.selectionStart, input.selectionEnd]).toEqual([2, 5]);
  expect(edit).not.toHaveBeenCalled();
  appearance.dispose();
  expect(listeners.size).toBe(0);
});
