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
/** Prove canvas preferences validate host values and persist only presentation choices. */

import { jest } from '@jest/globals';
import {
  ComfyCanvasPreferences,
  OUTPUT_CANVAS_VISIBLE_SETTING,
  OUTPUT_CANVAS_WIDTH_SETTING,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasPreferences.js';

interface Setting {
  readonly id: string;
  readonly defaultValue: boolean | number;
  readonly onChange: (value: unknown) => void;
}

test('registers, restores, and updates independent presentation settings with validated dynamic values', async () => {
  const registered = new Map<string, Setting>();
  const write = jest
    .fn<(id: string, value: boolean | number) => Promise<void>>()
    .mockImplementation(async (id, value) => {
      registered.get(id)!.onChange(value);
    });
  const preferences = new ComfyCanvasPreferences({
    ui: {
      settings: {
        addSetting: (setting: Setting) => registered.set(setting.id, setting),
        getSettingValue: (id: string) => (id === OUTPUT_CANVAS_VISIBLE_SETTING ? false : 620),
        setSettingValueAsync: write,
      },
    },
  });
  const changed = jest.fn();
  const stop = preferences.subscribe(changed);
  preferences.setup();
  preferences.setup();
  expect(registered.size).toBe(2);
  expect(preferences.snapshot).toEqual({ visible: false, width: 620 });
  for (const value of ['640', Infinity, NaN, -1, 20000, { width: 500 }])
    registered.get(OUTPUT_CANVAS_WIDTH_SETTING)!.onChange(value);
  registered.get(OUTPUT_CANVAS_VISIBLE_SETTING)!.onChange('true');
  expect(preferences.snapshot).toEqual({ visible: false, width: 620 });
  await preferences.visible(true);
  await preferences.width(700);
  expect(write.mock.calls).toEqual([
    [OUTPUT_CANVAS_VISIBLE_SETTING, true],
    [OUTPUT_CANVAS_WIDTH_SETTING, 700],
  ]);
  expect(preferences.snapshot).toEqual({ visible: true, width: 700 });
  stop();
  preferences.dispose();
  registered.get(OUTPUT_CANVAS_WIDTH_SETTING)!.onChange(500);
  expect(preferences.snapshot.width).toBe(700);
});

test('preserves accepted state on failed storage and ignores late completion after disposal', async () => {
  let complete: (() => void) | null = null;
  const preferences = new ComfyCanvasPreferences({
    ui: {
      settings: {
        addSetting: (_setting: Setting) => undefined,
        getSettingValue: (_id: string, fallback: unknown) => fallback,
        setSettingValueAsync: (_id: string, value: unknown) => {
          if (value === false) throw new Error('Storage failed.');
          return new Promise<void>((resolve) => {
            complete = resolve;
          });
        },
      },
    },
  });
  preferences.setup();
  await expect(preferences.visible(false)).rejects.toThrow('Storage failed.');
  expect(preferences.snapshot.visible).toBe(true);
  await expect(preferences.width(-1)).rejects.toThrow(RangeError);
  const pending = preferences.width(500);
  preferences.dispose();
  if (!complete) throw new Error('Fixture storage completion is missing.');
  const resolve: () => void = complete;
  resolve();
  await pending;
  expect(preferences.snapshot.width).toBe(0);
  expect(() => new ComfyCanvasPreferences({}).setup()).toThrow('unavailable');
});

test('follows host-local acceptance before persistence failure and ignores reordered storage completions', async () => {
  const registered = new Map<string, Setting>();
  const completions: (() => void)[] = [];
  const preferences = new ComfyCanvasPreferences({
    ui: {
      settings: {
        addSetting: (setting: Setting) => registered.set(setting.id, setting),
        getSettingValue: (_id: string, fallback: unknown) => fallback,
        setSettingValueAsync: (id: string, value: boolean | number) => {
          registered.get(id)!.onChange(value);
          if (value === false) return Promise.reject(new Error('Storage failed.'));
          return new Promise<void>((resolve) => completions.push(resolve));
        },
      },
    },
  });
  preferences.setup();
  await expect(preferences.visible(false)).rejects.toThrow('Storage failed.');
  expect(preferences.snapshot.visible).toBe(false);
  const earlier = preferences.width(500);
  const later = preferences.width(600);
  completions[1]!();
  await later;
  completions[0]!();
  await earlier;
  expect(preferences.snapshot.width).toBe(600);
  preferences.dispose();
});
