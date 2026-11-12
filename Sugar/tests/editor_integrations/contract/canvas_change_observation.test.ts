/** @jest-environment node */
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

/** Prove explicit observation negotiation, reference counting and teardown across both canvas protocols. */
import { jest } from '@jest/globals';
import { CanvasRemoteChanges } from '../../../integrations/shared/CanvasRemoteChanges.js';
import {
  CanvasChangeRelay,
  nativeCanvasChanges,
} from '../../../integrations/shared/CanvasChangeRelay.js';

/** Keep transport acknowledgements under test control while exercising actual lifetime ownership. */
function fixture() {
  const listeners = new Set<(change: number) => void>();
  const pending: ((supported: boolean) => void)[] = [];
  const watch = jest.fn(
    (enabled: boolean): Promise<boolean> =>
      enabled ? new Promise((resolve) => pending.push(resolve)) : Promise.resolve(true),
  );
  const report = jest.fn();
  const changes = new CanvasRemoteChanges<number>({
    watch,
    report,
    listen: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  });
  return {
    changes,
    watch,
    pending,
    report,
    listeners,
    emit: (change: number) => {
      for (const listener of listeners) listener(change);
    },
  };
}

test('shares one watch and preserves distinct early events until explicit acceptance', async () => {
  const f = fixture(),
    first = jest.fn(),
    second = jest.fn();
  const one = f.changes.subscribe(first),
    two = f.changes.subscribe(second);
  f.emit(1);
  f.emit(2);
  expect(first).not.toHaveBeenCalled();
  expect(f.watch.mock.calls).toEqual([[true]]);
  f.pending[0]!(true);
  expect(await one.ready).toBe('observing');
  expect(await two.ready).toBe('observing');
  expect(first.mock.calls).toEqual([[1], [2]]);
  expect(second.mock.calls).toEqual([[1], [2]]);
  one.stop();
  one.stop();
  expect(f.watch).toHaveBeenCalledTimes(1);
  two.stop();
  expect(f.watch.mock.calls).toEqual([[true], [false]]);
  expect(f.listeners.size).toBe(0);
  f.changes.dispose();
});

test('refuses an overflowing pending watch instead of silently dropping native changes', async () => {
  const f = fixture(),
    changed = jest.fn();
  const lease = f.changes.subscribe(changed);
  for (let index = 0; index < 65; index++) f.emit(index);
  f.pending[0]!(true);
  expect(await lease.ready).toBe('unavailable');
  expect(changed).not.toHaveBeenCalled();
  expect(f.listeners.size).toBe(0);
  expect(f.report).toHaveBeenCalledWith(
    expect.objectContaining({ message: expect.stringContaining('budget') }),
  );
  expect(f.watch.mock.calls).toEqual([[true], [false]]);
  lease.stop();
  f.changes.dispose();
  expect(f.watch.mock.calls).toEqual([[true], [false]]);
});

test('declares unsupported observation without accepting events or adding a retry or pixel timer', async () => {
  const f = fixture(),
    changed = jest.fn();
  const lease = f.changes.subscribe(changed);
  f.emit(1);
  f.pending[0]!(false);
  expect(await lease.ready).toBe('unsupported');
  f.emit(2);
  expect(changed).not.toHaveBeenCalled();
  expect(f.listeners.size).toBe(0);
  expect(f.watch.mock.calls).toEqual([[true]]);
  lease.stop();
  f.changes.dispose();
  expect(f.report).not.toHaveBeenCalled();
});

test('fences a pending watch after unmount without allowing its acknowledgement to retire a newer watch', async () => {
  const f = fixture(),
    old = jest.fn(),
    current = jest.fn();
  const before = f.changes.subscribe(old);
  before.stop();
  const after = f.changes.subscribe(current);
  f.pending[0]!(true);
  expect(await before.ready).toBe('unavailable');
  f.emit(3);
  expect(current).not.toHaveBeenCalled();
  f.pending[1]!(true);
  expect(await after.ready).toBe('observing');
  expect(current).toHaveBeenLastCalledWith(3);
  expect(old).not.toHaveBeenCalled();
  expect(f.watch.mock.calls).toEqual([[true], [false], [true]]);
  f.changes.dispose();
  f.emit(4);
  expect(current).toHaveBeenCalledTimes(1);
  expect(f.watch.mock.calls.at(-1)).toEqual([false]);
});

test('reports failed negotiation and releases a pending watch during connection disposal', async () => {
  const failure = new Error('Transport closed'),
    report = jest.fn();
  const watch = jest.fn(async (enabled: boolean) => {
    if (enabled) throw failure;
    return false;
  });
  const changes = new CanvasRemoteChanges<number>({
    watch,
    listen: () => () => {},
    report,
  });
  const lease = changes.subscribe(jest.fn());
  expect(await lease.ready).toBe('unavailable');
  expect(report).toHaveBeenCalledWith(failure);
  expect(watch.mock.calls).toEqual([[true], [false]]);
  changes.dispose();
  expect(await changes.subscribe(jest.fn()).ready).toBe('unavailable');
  const f = fixture(),
    pending = f.changes.subscribe(jest.fn());
  f.changes.dispose();
  f.pending[0]!(true);
  expect(await pending.ready).toBe('unavailable');
  expect(f.listeners.size).toBe(0);
  expect(f.watch.mock.calls).toEqual([[true], [false]]);
});

test('native relays reject unsupported observation and ignore callbacks after their session retires', async () => {
  const publish = jest.fn(),
    stop = jest.fn();
  const notifications: ((change: number) => void)[] = [];
  const relay = new CanvasChangeRelay(
    nativeCanvasChanges<number>((listener) => {
      notifications.push(listener);
      return stop;
    }),
    publish,
  );
  expect(await relay.watch(true)).toBe('observing');
  const emit = notifications[0];
  emit?.(1);
  relay.dispose();
  emit?.(2);
  expect(publish.mock.calls).toEqual([[1]]);
  expect(stop).toHaveBeenCalledTimes(1);
  expect(await relay.watch(true)).toBe('unavailable');
  const limited = new CanvasChangeRelay<number>(null, publish);
  expect(await limited.watch(true)).toBe('unsupported');
  limited.dispose();
});
