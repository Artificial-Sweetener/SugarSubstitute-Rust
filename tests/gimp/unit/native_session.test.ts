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
/** Prove cancellation survives native admission without replaying or undoing committed output. */
import { jest } from '@jest/globals';
import { NativeGimpSession } from '../../../integrations/gimp/src/application/NativeGimpSession.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import type { NativeResult } from '../../../integrations/gimp/src/contracts/NativeProtocol.generated.js';

/** Hold one external reply without timers or a browser. */
function deferred<T>() {
  let resolve: (value: T) => void = () => {
    throw new Error('Missing deferred resolver.');
  };
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

/** Supply typed native IO while retaining production session ownership. */
function fixture() {
  const ticket = deferred<number>();
  const response = deferred<NativeResult>();
  const submit = jest.fn(() => ticket.promise);
  const wait = jest.fn(() => response.promise);
  const cancel = jest.fn(async () => {});
  const close = jest.fn(async () => {});
  const report = jest.fn();
  const native = new NativeGimpSession({ submit, wait, cancel, close }, report);
  return { native, ticket, response, submit, wait, cancel, close, report };
}

test('does not submit an already-cancelled request', async () => {
  const f = fixture();
  const cancellation = new CanvasCancellation();
  cancellation.cancel();
  await expect(f.native.execute({ type: 'context' }, cancellation)).resolves.toMatchObject({
    status: 'cancelled',
  });
  expect(f.submit).not.toHaveBeenCalled();
  f.native.dispose();
});

test('forwards cancellation exactly once after delayed admission produces a ticket', async () => {
  const f = fixture();
  const cancellation = new CanvasCancellation();
  const result = f.native.execute({ type: 'context' }, cancellation);
  cancellation.cancel();
  expect(f.cancel).not.toHaveBeenCalled();
  f.ticket.resolve(7);
  f.response.resolve({ kind: 'released' });
  await expect(result).resolves.toMatchObject({ status: 'cancelled' });
  expect(f.cancel).toHaveBeenCalledTimes(1);
  expect(f.cancel).toHaveBeenCalledWith(7);
  expect(f.submit).toHaveBeenCalledTimes(1);
  f.native.dispose();
});

test.each<NativeResult>([
  { kind: 'inserted', committed: true, tattoo: 31, left: -1, top: 2, width: 3, height: 2 },
  { kind: 'channel-created', committed: true, tattoo: 32 },
  { kind: 'channel-selected', committed: true },
])('preserves a committed native mutation when cancellation arrives: %o', async (inserted) => {
  const f = fixture();
  const cancellation = new CanvasCancellation();
  const result = f.native.execute({ type: 'context' }, cancellation);
  f.ticket.resolve(7);
  await Promise.resolve();
  cancellation.cancel();
  f.response.resolve(inserted);
  await expect(result).resolves.toEqual(inserted);
  expect(f.cancel).toHaveBeenCalledTimes(1);
  f.native.dispose();
});

test('closing a renderer retires pending admission once', async () => {
  const f = fixture();
  const result = f.native.execute({ type: 'context' });
  f.native.dispose();
  f.native.dispose();
  f.ticket.resolve(7);
  f.response.resolve({ kind: 'released' });
  await expect(result).resolves.toMatchObject({ status: 'cancelled' });
  expect(f.close).toHaveBeenCalledTimes(1);
  expect(f.cancel).toHaveBeenCalledWith(7);
  await expect(f.native.execute({ type: 'context' })).resolves.toMatchObject({
    status: 'cancelled',
  });
  expect(f.submit).toHaveBeenCalledTimes(1);
});

test('reports cancellation delivery failures and returns cancellation without a retry', async () => {
  const f = fixture();
  const failure = new Error('Native pipe disconnected.');
  f.cancel.mockRejectedValue(failure);
  const cancellation = new CanvasCancellation();
  const result = f.native.execute({ type: 'context' }, cancellation);
  f.ticket.resolve(7);
  cancellation.cancel();
  f.response.resolve({ kind: 'released' });
  await expect(result).resolves.toMatchObject({ status: 'cancelled' });
  expect(f.report).toHaveBeenCalledWith(failure);
  expect(f.cancel).toHaveBeenCalledTimes(1);
  f.native.dispose();
});
