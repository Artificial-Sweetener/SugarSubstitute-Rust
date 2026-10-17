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
/** Characterize saved appearance preferences without modifying native values or workflow data. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  AppearanceHost,
  FLUENT_APPEARANCE_SETTING,
} from '../../../frontend/comfyui/ui/surface/appearance/AppearanceHost.js';
import { prepareNativePresentation } from '../../../frontend/comfyui/ui/surface/NativePresentationReady.js';

afterEach(() => {
  document.head.replaceChildren();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test.each([false, true])(
  'restores saved Fluent preference %s while retaining the native input',
  (enabled) => {
    jest.spyOn(window, 'requestAnimationFrame').mockImplementation(() => 1);
    jest.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
    const values = new Map<string, unknown>([
      ['Comfy.VueNodes.Enabled', true],
      [FLUENT_APPEARANCE_SETTING, enabled],
    ]);
    const settings = {
      addSetting: jest.fn(),
      getSettingValue: (key: string): unknown => values.get(key),
      setSettingValue: jest.fn(),
    };
    const card = document.createElement('div');
    card.className = 'lg-node';
    const input = document.createElement('input');
    input.value = 'native value <literal>';
    card.append(input);
    document.body.append(card);
    const errors = jest.fn();
    const host = new AppearanceHost({ document, app: { ui: { settings } }, reportError: errors });
    host.setup();
    prepareNativePresentation(card);
    expect(card.hasAttribute('data-sc-style-node')).toBe(enabled);
    expect(host.state.appearance).toEqual({ fluent: enabled, dark: true });
    expect(card.querySelector('input')).toBe(input);
    expect(input.value).toBe('native value <literal>');
    expect(settings.setSettingValue).not.toHaveBeenCalled();
    host.dispose();
    expect(card.hasAttribute('data-sc-style-node')).toBe(false);
    expect(errors).not.toHaveBeenCalled();
  },
);
