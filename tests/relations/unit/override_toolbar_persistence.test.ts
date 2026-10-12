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
const COLLAPSED_KEY = 'SugarCubes.GlobalOverrides.Collapsed';
afterEach(() => {
  localStorage.removeItem(KEY);
  localStorage.removeItem(COLLAPSED_KEY);
});

test('defaults to docked and restores floating coordinates and docking after recreation', () => {
  const logger = { warn: jest.fn() };
  const state = new NativeOverrideToolbarState(() => localStorage, logger);
  expect(state.read()).toEqual({ docked: true, floating: null, anchor: null });
  state.write({ docked: false, floating: { x: 120, y: 240 }, anchor: null });
  expect(new NativeOverrideToolbarState(() => localStorage, logger).read()).toEqual({
    docked: false,
    floating: { x: 120, y: 240 },
    anchor: null,
  });
  state.write({ docked: true, floating: { x: 120, y: 240 }, anchor: null });
  expect(new NativeOverrideToolbarState(() => localStorage, logger).read()).toEqual({
    docked: true,
    floating: { x: 120, y: 240 },
    anchor: null,
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
  expect(state.read()).toEqual({ docked: true, floating: null, anchor: null });
  expect(logger.warn).toHaveBeenCalledWith(
    expect.any(String),
    expect.objectContaining({ operation: 'read', error: expect.any(Error) }),
  );
});

test('keeps placement usable when browser storage is unavailable and reports access failures', () => {
  const logger = { warn: jest.fn() };
  const absent = new NativeOverrideToolbarState(() => null, logger);
  expect(absent.read()).toEqual({ docked: true, floating: null, anchor: null });
  absent.write({ docked: false, floating: null, anchor: null });
  expect(logger.warn).not.toHaveBeenCalled();
  const failure = new Error('Storage access denied');
  const blocked = new NativeOverrideToolbarState(() => {
    throw failure;
  }, logger);
  expect(blocked.read()).toEqual({ docked: true, floating: null, anchor: null });
  blocked.write({ docked: false, floating: { x: 1, y: 2 }, anchor: null });
  expect(logger.warn.mock.calls.map((call) => call[1])).toEqual([
    { operation: 'read', error: failure },
    { operation: 'write', error: failure },
  ]);
});

test('restores existing coordinate records and independently persists anchoring and disclosure', () => {
  const logger = { warn: jest.fn() };
  localStorage.setItem(KEY, JSON.stringify({ docked: false, floating: { x: 120, y: 240 } }));
  const state = new NativeOverrideToolbarState(() => localStorage, logger);
  expect(state.read()).toEqual({ docked: false, floating: { x: 120, y: 240 }, anchor: null });
  const placement = {
    docked: false,
    floating: { x: 120, y: 240 },
    anchor: { edge: 'left' as const, distance: 120, horizontalRatio: 0.1, verticalRatio: 0.3 },
  };
  state.write(placement);
  state.writeCollapsed(true);
  const restored = new NativeOverrideToolbarState(() => localStorage, logger);
  expect(restored.read()).toEqual(placement);
  expect(restored.readCollapsed()).toBe(true);
  restored.write({ ...placement, docked: true });
  expect(restored.readCollapsed()).toBe(true);
  restored.writeCollapsed(false);
  expect(restored.read().docked).toBe(true);
  expect(logger.warn).not.toHaveBeenCalled();
});

test.each([
  { edge: 'unknown', distance: 4, horizontalRatio: 0.1, verticalRatio: 0.3 },
  { edge: 'left', distance: -4, horizontalRatio: 0.1, verticalRatio: 0.3 },
  { edge: 'left', distance: 4, horizontalRatio: 5, verticalRatio: 0.3 },
  { edge: 'left', distance: 4, horizontalRatio: 0.1, verticalRatio: '0.3' },
])('rejects malformed anchoring metadata: %p', (anchor) => {
  localStorage.setItem(
    KEY,
    JSON.stringify({ docked: false, floating: { x: 120, y: 240 }, anchor }),
  );
  const logger = { warn: jest.fn() };
  expect(new NativeOverrideToolbarState(() => localStorage, logger).read()).toEqual({
    docked: true,
    floating: null,
    anchor: null,
  });
  expect(logger.warn).toHaveBeenCalledTimes(1);
});

test('validates disclosure storage and reports failed persistence without replacing placement', () => {
  const logger = { warn: jest.fn() };
  const state = new NativeOverrideToolbarState(() => localStorage, logger);
  expect(state.readCollapsed()).toBe(false);
  localStorage.setItem(COLLAPSED_KEY, '"true"');
  expect(state.readCollapsed()).toBe(false);
  expect(logger.warn).toHaveBeenLastCalledWith(
    expect.any(String),
    expect.objectContaining({ operation: 'readCollapsed' }),
  );
  const blocked = new NativeOverrideToolbarState(() => {
    throw new Error('Storage denied');
  }, logger);
  blocked.writeCollapsed(true);
  expect(logger.warn).toHaveBeenLastCalledWith(
    expect.any(String),
    expect.objectContaining({ operation: 'writeCollapsed' }),
  );
});
