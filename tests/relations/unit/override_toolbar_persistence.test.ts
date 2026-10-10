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
/** Prove browser-local placement defaults, validated restoration, and observable storage failures. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeOverrideToolbarState } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeOverrideToolbarState.js';

const KEY = 'SugarCubes.GlobalOverrides.Placement';
afterEach(() => localStorage.removeItem(KEY));

test('defaults to docked and restores floating coordinates and docking after recreation', () => {
  const logger = { warn: jest.fn() };
  const state = new NativeOverrideToolbarState(() => localStorage, logger);
  expect(state.read()).toEqual({ docked: true, floating: null });
  state.write({ docked: false, floating: { x: 120, y: 240 } });
  expect(new NativeOverrideToolbarState(() => localStorage, logger).read()).toEqual({
    docked: false,
    floating: { x: 120, y: 240 },
  });
  state.write({ docked: true, floating: { x: 120, y: 240 } });
  expect(new NativeOverrideToolbarState(() => localStorage, logger).read()).toEqual({
    docked: true,
    floating: { x: 120, y: 240 },
  });
  expect(logger.warn).not.toHaveBeenCalled();
});

test.each([
  '{',
  'null',
  '{"docked":"false","floating":null}',
  '{"docked":false,"floating":{"x":"10","y":20}}',
  '{"docked":true,"floating":{"x":1e400,"y":0}}',
])('rejects invalid persisted placement: %s', (raw) => {
  localStorage.setItem(KEY, raw);
  const logger = { warn: jest.fn() };
  const state = new NativeOverrideToolbarState(() => localStorage, logger);
  expect(state.read()).toEqual({ docked: true, floating: null });
  expect(logger.warn).toHaveBeenCalledWith(
    expect.any(String),
    expect.objectContaining({ operation: 'read', error: expect.any(Error) }),
  );
});

test('keeps placement usable when browser storage is unavailable and reports access failures', () => {
  const logger = { warn: jest.fn() };
  const absent = new NativeOverrideToolbarState(() => null, logger);
  expect(absent.read()).toEqual({ docked: true, floating: null });
  absent.write({ docked: false, floating: null });
  expect(logger.warn).not.toHaveBeenCalled();
  const failure = new Error('Storage access denied');
  const blocked = new NativeOverrideToolbarState(() => {
    throw failure;
  }, logger);
  expect(blocked.read()).toEqual({ docked: true, floating: null });
  blocked.write({ docked: false, floating: { x: 1, y: 2 } });
  expect(logger.warn.mock.calls.map((call) => call[1])).toEqual([
    { operation: 'read', error: failure },
    { operation: 'write', error: failure },
  ]);
});
