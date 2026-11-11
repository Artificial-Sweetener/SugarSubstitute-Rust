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
/** Prove embedded presentation overrides are local, reversible, and independent of saved changes. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { EmbeddedPresentation } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPresentation.js';
import {
  registerPluginModePreference,
  PLUGIN_MODE_SETTING,
} from '../../../frontend/comfyui/ui/surface/frame/PluginModePreference.js';
import {
  registerVisualStylePreference,
  VISUAL_STYLE_SETTING,
} from '../../../frontend/comfyui/ui/surface/appearance/VisualStylePreference.js';
import {
  registerPhotoProPalettePreference,
  PHOTOPRO_PALETTE_SETTING,
} from '../../../frontend/comfyui/ui/surface/appearance/PhotoProPalettePreference.js';
import { readPluginMode } from '../../../frontend/comfyui/ui/core/PluginMode.js';

afterEach(() => {
  document.documentElement.removeAttribute('data-sc-plugin-mode');
  document.documentElement.removeAttribute('data-sc-photopro-palette');
});

test('switches existing PhotoPro palettes live without changing other presentation preferences', () => {
  const embedded = new EmbeddedPresentation();
  const style = jest.fn();
  const mode = jest.fn();
  const writes = jest.fn();
  const app = {
    ui: {
      settings: {
        getSettingValue: (id: string): unknown =>
          id === PHOTOPRO_PALETTE_SETTING ? 'dark-gray' : false,
        addSetting: jest.fn(),
        setSettingValue: writes,
      },
    },
  };
  const stopPalette = registerPhotoProPalettePreference(app, document, jest.fn(), embedded);
  const stopStyle = registerVisualStylePreference(app, style, jest.fn(), embedded);
  const stopMode = registerPluginModePreference(app, document, mode, jest.fn(), embedded);
  embedded.set({
    pluginMode: true,
    visualStyle: 'photopro',
    photoProPalette: 'black',
    accentColor: null,
    resizeCornerSize: 0,
  });
  const styleCalls = style.mock.calls.length;
  const modeCalls = mode.mock.calls.length;
  for (const photoProPalette of ['dark-gray', 'medium-gray', 'light-gray', 'black'] as const) {
    embedded.set({
      pluginMode: true,
      visualStyle: 'photopro',
      photoProPalette,
      accentColor: null,
      resizeCornerSize: 0,
    });
    expect(document.documentElement.dataset.scPhotoproPalette).toBe(photoProPalette);
  }
  expect(style).toHaveBeenCalledTimes(styleCalls);
  expect(mode).toHaveBeenCalledTimes(modeCalls);
  expect(writes).not.toHaveBeenCalled();
  embedded.set(null);
  expect(document.documentElement.dataset.scPhotoproPalette).toBe('dark-gray');
  stopPalette();
  stopStyle();
  stopMode();
});

test('applies one session and restores the latest saved preferences without persistence writes', () => {
  const embedded = new EmbeddedPresentation();
  const changes = new Map<string, (value: unknown) => void>();
  const values = new Map<string, unknown>([
    [PLUGIN_MODE_SETTING, false],
    [VISUAL_STYLE_SETTING, 'fluent'],
    [PHOTOPRO_PALETTE_SETTING, 'black'],
  ]);
  const settings = {
    addSetting: (value: { id: string; onChange(value: unknown): void }): void => {
      changes.set(value.id, value.onChange);
    },
    getSettingValue: (id: string): unknown => values.get(id),
    setSettingValue: jest.fn(),
  };
  const app = { ui: { settings } };
  const style = jest.fn();
  const report = jest.fn();
  const stopMode = registerPluginModePreference(app, document, jest.fn(), report, embedded);
  const stopStyle = registerVisualStylePreference(app, style, report, embedded);
  const stopPalette = registerPhotoProPalettePreference(app, document, report, embedded);
  embedded.set({
    pluginMode: true,
    visualStyle: 'photopro',
    photoProPalette: 'medium-gray',
    accentColor: null,
    resizeCornerSize: 0,
  });
  expect(readPluginMode(document)).toBe(true);
  expect(style).toHaveBeenLastCalledWith('photopro');
  expect(document.documentElement.dataset.scPhotoproPalette).toBe('medium-gray');
  const calls = style.mock.calls.length;
  embedded.set({
    pluginMode: true,
    visualStyle: 'photopro',
    photoProPalette: 'medium-gray',
    accentColor: null,
    resizeCornerSize: 0,
  });
  expect(style).toHaveBeenCalledTimes(calls);
  changes.get(VISUAL_STYLE_SETTING)?.('spectrum');
  changes.get(PHOTOPRO_PALETTE_SETTING)?.('light-gray');
  changes.get(PLUGIN_MODE_SETTING)?.(false);
  expect(readPluginMode(document)).toBe(true);
  expect(style).toHaveBeenLastCalledWith('photopro');
  expect(document.documentElement.dataset.scPhotoproPalette).toBe('medium-gray');
  embedded.set(null);
  expect(readPluginMode(document)).toBe(false);
  expect(style).toHaveBeenLastCalledWith('spectrum');
  expect(document.documentElement.dataset.scPhotoproPalette).toBe('light-gray');
  expect(settings.setSettingValue).not.toHaveBeenCalled();
  expect(report).not.toHaveBeenCalled();
  stopMode();
  stopStyle();
  stopPalette();
  embedded.set({
    pluginMode: true,
    visualStyle: 'photopro',
    photoProPalette: 'black',
    accentColor: null,
    resizeCornerSize: 0,
  });
  expect(readPluginMode(document)).toBe(false);
  expect(document.documentElement.dataset.scPhotoproPalette).toBeUndefined();
});
