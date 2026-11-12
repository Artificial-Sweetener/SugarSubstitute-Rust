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
/** Prove the desktop host's theme reaches Comfy's palette without writing Comfy's settings. */
import { afterEach, beforeEach, expect, jest, test } from '@jest/globals';
import { HostColorPalette } from '../../../frontend/comfyui/ui/surface/appearance/HostColorPalette.js';

const DARK_PALETTES = new Set(['dark', 'nord']);

/** Stand in for Comfy's reactive settings store: palette values drive the root brightness class. */
function comfy(saved: string): {
  app: unknown;
  values: Record<string, unknown>;
  setSettingValue: jest.Mock;
} {
  const target: Record<string, unknown> = { 'Comfy.ColorPalette': saved };
  const apply = (id: unknown): void => {
    document.documentElement.classList.toggle('dark-theme', DARK_PALETTES.has(String(id)));
  };
  apply(saved);
  const values = new Proxy(target, {
    set(record, key, value) {
      record[String(key)] = value;
      if (key === 'Comfy.ColorPalette') apply(value);
      return true;
    },
  });
  const setSettingValue = jest.fn();
  return {
    app: {
      ui: {
        settings: {
          settingsValues: values,
          getSettingValue: (id: string) => target[id],
          setSettingValue,
        },
      },
    },
    values,
    setSettingValue,
  };
}

const dark = (): boolean => document.documentElement.classList.contains('dark-theme');

beforeEach(() => document.documentElement.classList.remove('dark-theme'));
afterEach(() => document.documentElement.classList.remove('dark-theme'));

test('a light host switches a dark Comfy palette to the shipped light palette in memory only', () => {
  const { app, values, setSettingValue } = comfy('dark');
  const report = jest.fn();
  const palette = new HostColorPalette(document, app, report);
  palette.follow(false);
  expect(values['Comfy.ColorPalette']).toBe('light');
  expect(dark()).toBe(false);
  expect(setSettingValue).not.toHaveBeenCalled();
  expect(report).not.toHaveBeenCalled();
});

test('follows every host change in both directions', () => {
  const { app, values } = comfy('dark');
  const palette = new HostColorPalette(document, app, jest.fn());
  for (const hostDark of [false, true, false, true]) {
    palette.follow(hostDark);
    expect(values['Comfy.ColorPalette']).toBe(hostDark ? 'dark' : 'light');
    expect(dark()).toBe(hostDark);
  }
});

test('keeps a saved palette whose brightness already matches the host', () => {
  const { app, values } = comfy('nord');
  const palette = new HostColorPalette(document, app, jest.fn());
  palette.follow(true);
  palette.reconcile();
  expect(values['Comfy.ColorPalette']).toBe('nord');
});

test('reasserts the host theme when Comfy later changes brightness', () => {
  const { app, values } = comfy('dark');
  const palette = new HostColorPalette(document, app, jest.fn());
  palette.follow(false);
  values['Comfy.ColorPalette'] = 'dark';
  expect(dark()).toBe(true);
  palette.reconcile();
  expect(values['Comfy.ColorPalette']).toBe('light');
  expect(dark()).toBe(false);
});

test('does not write the palette before a host theme is accepted', () => {
  const { app, values } = comfy('dark');
  const palette = new HostColorPalette(document, app, jest.fn());
  palette.reconcile();
  expect(values['Comfy.ColorPalette']).toBe('dark');
});

test('release restores the palette that was saved before the override', () => {
  const { app, values } = comfy('nord');
  const palette = new HostColorPalette(document, app, jest.fn());
  palette.follow(false);
  expect(values['Comfy.ColorPalette']).toBe('light');
  palette.release();
  expect(values['Comfy.ColorPalette']).toBe('nord');
  expect(dark()).toBe(true);
  palette.reconcile();
  expect(values['Comfy.ColorPalette']).toBe('nord');
});

test('release restores the palette the user chose while the host overrode it', () => {
  const { app, values } = comfy('dark');
  const palette = new HostColorPalette(document, app, jest.fn());
  palette.follow(true);
  values['Comfy.ColorPalette'] = 'solarized-light';
  expect(dark()).toBe(false);
  palette.reconcile();
  expect(values['Comfy.ColorPalette']).toBe('dark');
  palette.release();
  expect(values['Comfy.ColorPalette']).toBe('solarized-light');
});

test('reports an unavailable Comfy settings surface once instead of ignoring the host theme', () => {
  const report = jest.fn();
  const palette = new HostColorPalette(document, { ui: { settings: {} } }, report);
  palette.follow(false);
  palette.reconcile();
  expect(report).toHaveBeenCalledTimes(1);
  expect(report.mock.calls[0]?.[0]).toBeInstanceOf(Error);
});
