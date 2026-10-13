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
/** Verify browser-backed port qualification matches the reusable overlay attachment contract. */

import { jest } from '@jest/globals';
import { BrowserCanvasPort } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasPort.js';
import { canvasSnapshot } from '../support/CanvasPortFixture.js';
import type {
  CanvasCommandRequest,
  CanvasSurfaceGeometry,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';

/** Supply explicit ownership collaborators without image or host runtime dependencies. */
function fixture() {
  const initial = canvasSnapshot('browser-port');
  let current = true;
  const apply = jest.fn<() => boolean | Promise<boolean>>().mockReturnValue(true);
  const geometry = jest.fn<(value: CanvasSurfaceGeometry) => void>();
  const release = jest.fn<() => void>();
  const report = jest.fn<(error: unknown, request: CanvasCommandRequest) => void>();
  const port = new BrowserCanvasPort(initial, {
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
