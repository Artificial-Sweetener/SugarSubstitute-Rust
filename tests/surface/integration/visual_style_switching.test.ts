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
/** Prove visual-style persistence, renderer transitions, and native value preservation. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { AppearanceHost } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceHost.js';
import { VISUAL_STYLE_SETTING } from '../../../frontend/comfyui/ui/surface/appearance/VisualStylePreference.js';
import { prepareNativePresentation } from '../../../frontend/comfyui/ui/surface/NativePresentationReady.js';
import { isRecord, type UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';
import type { VisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyle.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  document.documentElement.removeAttribute('data-sc-visual-style');
  jest.restoreAllMocks();
});

/** Retain real native controls and expose only the host settings/event boundary. */
function fixture(style: VisualStyle) {
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  jest.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
  const values = new Map<string, unknown>([
    [VISUAL_STYLE_SETTING, style],
    ['Comfy.VueNodes.Enabled', true],
    ['SugarCubes.Appearance.Fluent', true],
  ]);
  const registrations = new Map<string, UnknownRecord>();
  const events = new EventTarget();
  const errors = jest.fn();
  const root = document.createElement('div');
  root.className = 'lg-node';
  const input = document.createElement('input');
  input.value = '<b>literal native value</b>';
  root.append(input);
  document.body.append(root);
  const settings = {
    getSettingValue: (key: string): unknown => values.get(key),
    addSetting: (value: unknown): void => {
      if (!isRecord(value) || typeof value.id !== 'string')
        throw new TypeError('Invalid setting registration.');
      registrations.set(value.id, value);
    },
    addEventListener: events.addEventListener.bind(events),
    removeEventListener: events.removeEventListener.bind(events),
  };
  const host = new AppearanceHost({ document, app: { ui: { settings } }, reportError: errors });
  host.setup();
  prepareNativePresentation(root);
  return { host, root, input, values, registrations, events, frames, errors };
}

test.each(['comfy', 'fluent', 'spectrum'] as const)(
  'restores %s from the combobox without following the superseded legacy toggle',
  (style) => {
    const f = fixture(style);
    const setting = f.registrations.get(VISUAL_STYLE_SETTING);
    expect(setting?.type).toBe('combo');
    expect(setting?.options).toEqual([
      { value: 'comfy', text: 'Comfy' },
      { value: 'fluent', text: 'Fluent' },
      { value: 'spectrum', text: 'Spectrum 2' },
    ]);
    expect(f.registrations.has('SugarCubes.Appearance.Fluent')).toBe(false);
    expect(f.host.state.appearance.style).toBe(style);
    expect(document.documentElement.getAttribute('data-sc-visual-style')).toBe(style);
    expect(f.root.hasAttribute('data-sc-style-node')).toBe(style !== 'comfy');
    expect(f.root.querySelector('input')).toBe(f.input);
    expect(f.input.value).toBe('<b>literal native value</b>');
    expect(f.root.querySelector('b')).toBeNull();
    f.host.dispose();
    expect(document.documentElement.hasAttribute('data-sc-visual-style')).toBe(false);
    expect(f.errors).not.toHaveBeenCalled();
  },
);

test('switches both providers reversibly and preserves Spectrum preference across renderer suspension', async () => {
  const f = fixture('fluent');
  for (const style of ['spectrum', 'fluent', 'comfy', 'spectrum'] as const) {
    f.host.setStyle(style);
    prepareNativePresentation(f.root);
    expect(f.root.querySelector('input')).toBe(f.input);
    expect(f.host.state.appearance.style).toBe(style);
  }
  f.values.set('Comfy.VueNodes.Enabled', false);
  f.events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
  await Promise.resolve();
  expect(f.host.state.appearance.style).toBe('comfy');
  expect(f.root.hasAttribute('data-sc-style-node')).toBe(false);
  expect(document.documentElement.getAttribute('data-sc-visual-style')).toBe('spectrum');
  f.values.set('Comfy.VueNodes.Enabled', true);
  f.events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
  await Promise.resolve();
  prepareNativePresentation(f.root);
  expect(f.host.state.appearance.style).toBe('spectrum');
  expect(document.getElementById('sugarcubes-native-appearance')?.textContent).toContain(
    '--sc-node-card-radius:var(--spectrum-corner-radius-large-default)',
  );
  f.host.dispose();
  expect(f.errors).not.toHaveBeenCalled();
});

test('rejects an invalid combobox value without changing accepted appearance', () => {
  const f = fixture('spectrum');
  const change = f.registrations.get(VISUAL_STYLE_SETTING)?.onChange;
  if (typeof change !== 'function') throw new Error('Visual-style change callback is unavailable.');
  change('<img src=x>');
  expect(f.host.state.appearance.style).toBe('spectrum');
  expect(f.errors).toHaveBeenCalledWith(expect.any(TypeError));
  f.host.dispose();
});
