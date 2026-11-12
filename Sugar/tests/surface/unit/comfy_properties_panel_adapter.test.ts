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
/** Prove Properties requests are constrained before native state, callbacks and persistence. */
import { expect, jest, test } from '@jest/globals';
import { ComfyPropertiesPanelAdapter } from '../../../frontend/comfyui/ui/surface/ComfyPropertiesPanelAdapter.js';

const PANEL = 'Comfy.RightSidePanel.IsOpen';

/** Model the public native setting contract and its synchronous pre-commit normalization. */
function fixture(open = false) {
  const values: Record<string, unknown> = { [PANEL]: open };
  const definition: { migrateDeprecatedValue?: (value: unknown) => unknown } = {};
  const committed = jest.fn();
  const reportError = jest.fn();
  const settings = {
    settings: { [PANEL]: definition },
    get: (id: string) => values[id],
    async set(id: string, value: unknown) {
      const next = id === PANEL ? (definition.migrateDeprecatedValue?.(value) ?? value) : value;
      if (next === values[id]) return;
      committed(id, next);
      values[id] = next;
    },
  };
  const adapter = new ComfyPropertiesPanelAdapter(
    { extensionManager: { setting: settings } },
    reportError,
  );
  adapter.setup();
  return { adapter, settings, values, definition, committed, reportError };
}

test('closes immediately and never commits or persists an opening request while blocked', async () => {
  const f = fixture(true);
  f.adapter.update(true);
  expect(f.values[PANEL]).toBe(false);
  expect(f.committed).toHaveBeenCalledWith(PANEL, false);
  f.committed.mockClear();
  for (let attempt = 0; attempt < 4; attempt += 1) await f.settings.set(PANEL, true);
  expect(f.values[PANEL]).toBe(false);
  expect(f.committed).not.toHaveBeenCalled();
  await f.settings.set('Comfy.Sidebar.Location', 'right');
  expect(f.values['Comfy.Sidebar.Location']).toBe('right');
  f.adapter.update(false);
  expect(f.values[PANEL]).toBe(true);
  await f.settings.set(PANEL, true);
  expect(f.values[PANEL]).toBe(true);
  f.adapter.dispose();
  expect(Object.hasOwn(f.definition, 'migrateDeprecatedValue')).toBe(false);
});

test.each([false, true])(
  'restores the saved %s state across repeated visits and disposal',
  async (open) => {
    const f = fixture(open);
    for (let visit = 0; visit < 3; visit += 1) {
      f.adapter.update(true);
      f.adapter.update(true);
      await f.settings.set(PANEL, true);
      expect(f.values[PANEL]).toBe(false);
      f.adapter.update(false);
      expect(f.values[PANEL]).toBe(open);
      f.committed.mockClear();
      f.adapter.update(false);
      expect(f.committed).not.toHaveBeenCalled();
    }
    await f.settings.set(PANEL, !open);
    f.adapter.update(true);
    f.adapter.dispose();
    expect(f.values[PANEL]).toBe(!open);
    expect(f.reportError).not.toHaveBeenCalled();
  },
);

test('preserves host normalization and restores its exact hook on disposal', () => {
  const definition: { migrateDeprecatedValue: (value: unknown) => unknown } = {
    migrateDeprecatedValue: jest.fn((value: unknown) => value === 'open'),
  };
  const original = definition.migrateDeprecatedValue;
  const settings = { settings: { [PANEL]: definition }, get: () => false, set: jest.fn() };
  const adapter = new ComfyPropertiesPanelAdapter(
    { extensionManager: { setting: settings } },
    jest.fn(),
  );
  adapter.setup();
  expect(definition.migrateDeprecatedValue('open')).toBe(true);
  adapter.update(true);
  expect(definition.migrateDeprecatedValue('open')).toBe(false);
  adapter.update(false);
  expect(definition.migrateDeprecatedValue('open')).toBe(true);
  adapter.dispose();
  expect(definition.migrateDeprecatedValue).toBe(original);
  const replacement = () => false;
  adapter.setup();
  definition.migrateDeprecatedValue = replacement;
  adapter.dispose();
  expect(definition.migrateDeprecatedValue).toBe(replacement);
});

test('reports synchronous and asynchronous native close failures without losing the guard', async () => {
  for (const asynchronous of [false, true]) {
    const failure = new Error('Native persistence failed.');
    const definition: { migrateDeprecatedValue?: (value: unknown) => unknown } = {};
    const report = jest.fn();
    const settings = {
      settings: { [PANEL]: definition },
      get: () => true,
      set() {
        if (asynchronous) return Promise.reject(failure);
        throw failure;
      },
    };
    const adapter = new ComfyPropertiesPanelAdapter(
      { extensionManager: { setting: settings } },
      report,
    );
    expect(adapter.update(true)).toBe(false);
    await Promise.resolve();
    expect(report).toHaveBeenCalledWith(failure);
    expect(definition.migrateDeprecatedValue?.(true)).toBe(false);
    adapter.dispose();
  }
});

test('reports missing modern definitions and accepts hosts without the Properties setting surface', () => {
  const report = jest.fn();
  const adapter = new ComfyPropertiesPanelAdapter(
    { extensionManager: { setting: { get: () => true, set() {}, settings: {} } } },
    report,
  );
  expect(adapter.update(true)).toBe(false);
  expect(report).toHaveBeenCalledWith(
    expect.objectContaining({ message: 'Comfy Properties setting definition is unavailable.' }),
  );
  adapter.dispose();
  const legacy = new ComfyPropertiesPanelAdapter({}, report);
  legacy.update(true);
  legacy.dispose();
  expect(report).toHaveBeenCalledTimes(1);
});

test.each([false, true])(
  'reports restoration failure once when asynchronous is %s',
  async (asynchronous) => {
    const f = fixture(true);
    expect(f.adapter.update(true)).toBe(true);
    const failure = new Error('Native restore failed.');
    f.settings.set = () => {
      if (asynchronous) return Promise.reject(failure);
      throw failure;
    };
    expect(f.adapter.update(false)).toBe(true);
    await Promise.resolve();
    expect(f.reportError).toHaveBeenCalledWith(failure);
    f.adapter.update(false);
    f.adapter.dispose();
    expect(f.reportError).toHaveBeenCalledTimes(1);
  },
);
