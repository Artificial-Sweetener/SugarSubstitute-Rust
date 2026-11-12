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
/** Verify surface-independent qualification for the reusable overlay attachment contract. */

import { jest } from '@jest/globals';
import { CanvasSurfacePort } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfacePort.js';
import { canvasSnapshot } from '../support/CanvasPortFixture.js';
import type {
  CanvasCommand,
  CanvasCommandRequest,
  CanvasCommandScope,
  CanvasSurfaceGeometry,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';

/** Supply explicit ownership collaborators without image or host runtime dependencies. */
function fixture(name = 'surface-port', revision = 1) {
  const initial = { ...canvasSnapshot(name), revision };
  let current = true;
  const apply = jest
    .fn<(command: CanvasCommand, scope: CanvasCommandScope) => boolean | Promise<boolean>>()
    .mockReturnValue(true);
  const geometry = jest.fn<(value: CanvasSurfaceGeometry) => void>();
  const release = jest.fn<() => void>();
  const report = jest.fn<(error: unknown, request: CanvasCommandRequest) => void>();
  const port = new CanvasSurfacePort(initial, {
    origin: 'http://localhost',
    capabilities: { schema: 1, commands: ['fit', 'zoom'] },
    accepts: () => current,
    apply,
    geometry,
    release,
    report,
  });
  const request: CanvasCommandRequest = {
    schema: 1,
    address: initial.address,
    expectedRevision: initial.revision,
    requestId: 1,
    command: { type: 'fit' },
  };
  return {
    initial,
    port,
    request,
    apply,
    geometry,
    release,
    report,
    retire: () => {
      current = false;
    },
  };
}

test('keeps simultaneous surface attachments independent during a delayed command', async () => {
  const previous = fixture('previous-surface');
  const replacement = fixture('replacement-surface');
  let resolve: ((value: boolean) => void) | undefined;
  previous.apply.mockImplementationOnce(
    () =>
      new Promise<boolean>((complete) => {
        resolve = complete;
      }),
  );
  const pending = previous.port.dispatch(previous.request);
  replacement.port.publish(replacement.initial);
  expect(previous.port.read().revision).toBe(previous.initial.revision);
  expect(
    (await replacement.port.dispatch({ ...replacement.request, expectedRevision: 2 })).status,
  ).toBe('applied');
  previous.retire();
  if (!resolve) throw new Error('Missing asynchronous command resolver');
  resolve(true);
  expect((await pending).status).toBe('cancelled');
  previous.port.releaseSurface(replacement.initial.address);
  expect(previous.release).not.toHaveBeenCalled();
  replacement.port.releaseSurface(replacement.initial.address);
  expect(replacement.release).toHaveBeenCalledTimes(1);
  previous.port.dispose();
  replacement.port.dispose();
});

test('rejects duplicates, old revisions, foreign addresses, and unsupported/malformed commands', async () => {
  const f = fixture();
  expect((await f.port.dispatch(f.request)).status).toBe('applied');
  expect((await f.port.dispatch(f.request)).status).toBe('stale');
  expect(
    (
      await f.port.dispatch({
        ...f.request,
        requestId: 2,
        address: canvasSnapshot('foreign').address,
      })
    ).status,
  ).toBe('unsupported');
  expect(
    (await f.port.dispatch({ ...f.request, requestId: 2, command: { type: 'zoom', factor: NaN } }))
      .status,
  ).toBe('unsupported');
  f.port.publish({
    ...f.initial,
    revision: 999,
    resource: { ...f.initial.resource, message: 'Current' },
  });
  expect(f.port.read().revision).toBe(2);
  expect((await f.port.dispatch({ ...f.request, requestId: 2 })).status).toBe('stale');
  expect((await f.port.dispatch({ ...f.request, requestId: 2, expectedRevision: 2 })).status).toBe(
    'applied',
  );
  expect(f.apply).toHaveBeenCalledTimes(2);
  f.port.dispose();
});

test('allocates a single nonwrapping request sequence for all attachment consumers', async () => {
  const f = fixture();
  const first = f.port.createRequest({ type: 'fit' }, f.initial.revision);
  const second = f.port.createRequest({ type: 'fit' }, f.initial.revision);
  expect([first.requestId, second.requestId]).toEqual([1, 2]);
  expect((await f.port.dispatch(second)).status).toBe('applied');
  expect((await f.port.dispatch(first)).status).toBe('stale');
  expect((await f.port.dispatch({ ...f.request, requestId: 50 })).status).toBe('applied');
  f.port.publish(f.initial);
  const next = f.port.createRequest({ type: 'fit' }, f.port.read().revision);
  expect(next.requestId).toBe(51);
  expect(next.expectedRevision).toBe(2);
  expect((await f.port.dispatch({ ...next, requestId: Number.MAX_SAFE_INTEGER })).status).toBe(
    'applied',
  );
  expect(() => f.port.createRequest({ type: 'fit' }, 2)).toThrow(
    'Canvas request identity exhausted.',
  );
  f.port.dispose();
  expect(() => f.port.createRequest({ type: 'fit' }, 2)).toThrow(
    'Canvas attachment is unavailable.',
  );
});

test('rejects revision exhaustion before changing or publishing the retained snapshot', () => {
  const f = fixture('last-revision', Number.MAX_SAFE_INTEGER);
  const listener = jest.fn();
  f.port.subscribe(listener);
  expect(() => f.port.publish(f.initial)).toThrow('Canvas surface revision exhausted.');
  expect(f.port.read()).toBe(f.initial);
  expect(f.port.read().revision).toBe(Number.MAX_SAFE_INTEGER);
  expect(listener).not.toHaveBeenCalled();
  f.port.dispose();
});

test.each([-1, Number.MAX_SAFE_INTEGER + 1, 0.5, NaN])(
  'rejects an invalid initial surface revision: %s',
  (revision) => {
    expect(() => fixture('invalid-revision', revision)).toThrow(
      'Canvas surface revision is invalid.',
    );
  },
);

test.each(['superseded', 'retired'] as const)(
  'qualifies asynchronous command completion after it is %s',
  async (reason) => {
    const f = fixture();
    let resolve: ((value: boolean) => void) | undefined;
    f.apply.mockImplementationOnce(
      () =>
        new Promise<boolean>((complete) => {
          resolve = complete;
        }),
    );
    const pending = f.port.dispatch(f.request);
    if (reason === 'superseded')
      expect((await f.port.dispatch({ ...f.request, requestId: 2 })).status).toBe('applied');
    else f.retire();
    if (!resolve) throw new Error('Missing asynchronous command resolver');
    resolve(true);
    expect((await pending).status).toBe(reason === 'superseded' ? 'stale' : 'cancelled');
    f.port.dispose();
  },
);

test('passes the admitted identity and cancels stale work before its surface mutation', async () => {
  const f = fixture();
  const mutations: number[] = [];
  let release!: () => void;
  let first!: CanvasCommandScope;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  f.apply.mockImplementation(async (_command, scope) => {
    if (scope.request.requestId === 1) {
      first = scope;
      await gate;
    }
    if (!scope.current()) return false;
    mutations.push(scope.request.requestId);
    return true;
  });
  const pending = f.port.dispatch(f.request);
  expect(first.request).toEqual(f.request);
  expect(Object.isFrozen(first.request)).toBe(true);
  expect(first.current()).toBe(true);
  expect(
    (
      await f.port.dispatch({
        ...f.request,
        requestId: 2,
        address: canvasSnapshot('other').address,
      })
    ).status,
  ).toBe('unsupported');
  expect(first.current()).toBe(true);
  expect((await f.port.dispatch({ ...f.request, requestId: 2 })).status).toBe('applied');
  expect(first.signal.aborted).toBe(true);
  release();
  expect((await pending).status).toBe('stale');
  expect(mutations).toEqual([2]);
  f.port.dispose();
});

test('reentrant cancellation cannot let an intermediate command reach its surface', async () => {
  const f = fixture();
  let nested: Promise<
    import('../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js').CanvasCommandOutcome
  > | null = null;
  const applied: number[] = [];
  f.apply.mockImplementation((_command, scope) => {
    if (scope.request.requestId === 1) {
      return new Promise<boolean>((resolve) => {
        scope.signal.addEventListener('abort', () => {
          nested = f.port.dispatch({ ...f.request, requestId: 3 });
          resolve(false);
        });
      });
    }
    applied.push(scope.request.requestId);
    return true;
  });
  const first = f.port.dispatch(f.request);
  const second = f.port.dispatch({ ...f.request, requestId: 2 });
  expect((await second).status).toBe('stale');
  expect(nested).not.toBeNull();
  expect((await nested!)?.status).toBe('applied');
  expect((await first).status).toBe('stale');
  expect(applied).toEqual([3]);
  f.port.dispose();
});

test('retirement aborts pending work and the original scope cannot outlive a completed command', async () => {
  const f = fixture();
  let held!: CanvasCommandScope;
  let release!: () => void;
  f.apply.mockImplementationOnce(async (_command, scope) => {
    held = scope;
    await new Promise<void>((resolve) => {
      release = resolve;
    });
    return scope.current();
  });
  const pending = f.port.dispatch(f.request);
  f.port.dispose();
  expect(held.signal.aborted).toBe(true);
  expect(held.current()).toBe(false);
  release();
  expect((await pending).status).toBe('cancelled');

  const next = fixture('completed');
  next.apply.mockImplementationOnce((_command, scope) => {
    held = scope;
    return true;
  });
  expect((await next.port.dispatch(next.request)).status).toBe('applied');
  expect(held.current()).toBe(false);
  expect(held.signal.aborted).toBe(true);
  next.port.dispose();
});

test('rejects inactive workflow generations and reports original command failure context', async () => {
  const f = fixture();
  const error = new Error('Camera owner failed.');
  f.apply.mockImplementation(() => {
    throw error;
  });
  expect((await f.port.dispatch(f.request)).status).toBe('error');
  expect(f.report).toHaveBeenCalledWith(
    error,
    expect.objectContaining({ address: f.initial.address }),
  );
  f.retire();
  expect((await f.port.dispatch({ ...f.request, requestId: 2 })).status).toBe('cancelled');
  f.port.releaseSurface(f.initial.address);
  expect(f.release).not.toHaveBeenCalled();
  f.port.dispose();
});

test('validates surface geometry and scopes subscription and release to the current attachment', () => {
  const f = fixture();
  const changed = jest.fn();
  const stop = f.port.subscribe(changed);
  const geometry: CanvasSurfaceGeometry = {
    coordinateSpace: 'client',
    units: 'css-pixels',
    origin: { x: 0, y: 0 },
    viewport: { width: 800, height: 600 },
    clip: { x: 0, y: 0, width: 800, height: 600 },
    physicalPixelsPerCssPixel: 2,
    visible: true,
    focused: false,
  };
  f.port.updateSurface(f.initial.address, geometry);
  f.port.updateSurface(f.initial.address, { ...geometry, physicalPixelsPerCssPixel: Infinity });
  f.port.updateSurface(canvasSnapshot('foreign').address, geometry);
  expect(f.geometry).toHaveBeenCalledTimes(1);
  f.port.publish(f.initial);
  expect(changed).toHaveBeenCalledTimes(1);
  stop();
  f.port.publish(f.initial);
  expect(changed).toHaveBeenCalledTimes(1);
  f.port.releaseSurface(canvasSnapshot('foreign').address);
  expect(f.release).not.toHaveBeenCalled();
  f.port.releaseSurface(f.initial.address);
  expect(f.release).toHaveBeenCalledTimes(1);
  f.port.dispose();
  f.port.releaseSurface(f.initial.address);
  expect(f.release).toHaveBeenCalledTimes(1);
});
