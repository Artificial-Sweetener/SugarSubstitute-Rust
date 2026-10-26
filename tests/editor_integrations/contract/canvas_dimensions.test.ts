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
/** Prove shared dimension arithmetic and geometry transport across capable and limited canvases. */
import { jest } from '@jest/globals';
import {
  canvasSizingProposal,
  constrainCanvasDimension,
} from '../../../integrations/shared/CanvasDimensions.js';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { WEB_CANVAS } from '../../../integrations/contracts/CanvasHost.js';
import { isCanvasGeometryCommand } from '../../../integrations/shared/CanvasValidation.generated.js';
import type {
  CanvasGeometryCommand,
  CanvasGeometryResult,
  CanvasMessage,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasDimensionConfiguration } from '../../../integrations/contracts/CanvasGeometry.js';
import type { CanvasCancellationSource } from '../../../integrations/contracts/CanvasHost.js';

const document = { id: 'document', geometryRevision: 'geometry', width: 1920, height: 1080 };
const limits = { minimum: 16, maximum: 16384, multiple: 8 };
const proposal: CanvasDimensionConfiguration = {
  document,
  direction: 'from-target',
  reference: { width: document.width, height: document.height },
  generation: { width: 1024, height: 1024 },
  widthLimits: limits,
  heightLimits: limits,
  stage: 'factor',
  scale: '2',
  outputs: [],
  output: null,
};

test('divides incoming references, multiplies outgoing dimensions and respects destination field grids', () => {
  expect(canvasSizingProposal(proposal)).toEqual({ width: 960, height: 544 });
  expect(
    canvasSizingProposal({
      ...proposal,
      direction: 'to-canvas',
      widthLimits: { ...limits, multiple: 1 },
      heightLimits: { ...limits, multiple: 1 },
    }),
  ).toEqual({ width: 2048, height: 2048 });
  expect(constrainCanvasDimension(3, limits)).toBe(16);
  expect(constrainCanvasDimension(20000, limits)).toBe(16384);
});

test.each(['0', '-2', 'Infinity', 'NaN', '1001'])('rejects invalid scale factors: %s', (scale) => {
  expect(() => canvasSizingProposal(proposal, scale)).toThrow('scale factor');
});

test('rejects impossible grids rather than producing an invalid native field value', () => {
  expect(() => constrainCanvasDimension(4, { minimum: 1, maximum: 3, multiple: 8 })).toThrow();
  expect(() => constrainCanvasDimension(4, { ...limits, multiple: 0 })).toThrow();
});

test('calculates from automatic field values while canvas mutations require positive dimensions', () => {
  expect(canvasSizingProposal({ ...proposal, generation: { width: 0, height: 0 } })).toEqual({
    width: 960,
    height: 544,
  });
  expect(isCanvasGeometryCommand({ type: 'resize', document, size: { width: 0, height: 0 } })).toBe(
    false,
  );
  expect(
    isCanvasGeometryCommand({
      type: 'resize',
      document: { ...document, width: 0 },
      size: { width: 100, height: 100 },
    }),
  ).toBe(false);
});

/** Exercise real shared correlation and replay owners with one replaceable native capability. */
function fixture(capable: boolean) {
  const sent: CanvasMessage[] = [],
    errors: unknown[] = [];
  let cancel: CanvasCancellationSource | undefined;
  const execute = jest.fn(
    async (
      _command: CanvasGeometryCommand,
      cancellation?: CanvasCancellationSource,
    ): Promise<CanvasGeometryResult> => {
      cancel = cancellation;
      return { kind: 'resized', document: { ...document, width: 1024, height: 1024 } };
    },
  );
  const server = new CanvasServer('canvas-dimensions-session', {
    ...(capable ? { geometry: { canvasId: 'test-editor', execute } } : {}),
    context: () => ({ canvas: WEB_CANVAS, document: null }),
    insert: async () => ({ status: 'unsupported', message: 'No insertion.' }),
    send: (reply) => client.receive(reply),
    report: (error) => errors.push(error),
  });
  const client = new CanvasClient('canvas-dimensions-session', {
    send: (message) => {
      sent.push(message);
      server.receive(message);
    },
    later: () => () => {},
    report: (error) => errors.push(error),
  });
  return { client, server, execute, sent, errors, cancelled: () => cancel?.cancelled() };
}

test.each([true, false])(
  'negotiates a geometry capability without changing the limited canvas: %s',
  async (capable) => {
    const f = fixture(capable);
    const result = await f.client.geometry.execute({
      type: 'resize',
      document,
      size: { width: 1024, height: 1024 },
    });
    expect(result).toMatchObject(
      capable ? { kind: 'resized' } : { kind: 'failure', status: 'unsupported' },
    );
    expect(f.execute).toHaveBeenCalledTimes(capable ? 1 : 0);
    if (capable) {
      f.server.receive(f.sent[0]);
      expect(f.execute).toHaveBeenCalledTimes(1);
      f.server.receive({
        ...f.sent[0],
        command: { type: 'resize', document, size: { width: 99, height: 99 } },
      });
      expect(f.execute).toHaveBeenCalledTimes(1);
      f.server.receive({
        ...f.sent[0],
        requestId: 2,
        command: { type: 'resize', document, size: { width: -1, height: 99 } },
      });
      expect(f.execute).toHaveBeenCalledTimes(1);
    }
    f.client.dispose();
    f.server.dispose();
    expect(f.errors).toEqual([]);
  },
);

test('cancels native geometry ownership when its navigation leaves', async () => {
  const f = fixture(true);
  let finish: ((result: CanvasGeometryResult) => void) | undefined;
  f.execute.mockImplementation((_command, cancellation) => {
    cancellation?.subscribe(() =>
      finish?.({ kind: 'failure', status: 'cancelled', message: 'Closed.' }),
    );
    return new Promise((resolve) => {
      finish = resolve;
    });
  });
  const pending = f.client.geometry.execute({
    type: 'resize',
    document,
    size: { width: 1000, height: 1000 },
  });
  expect(f.execute).toHaveBeenCalledTimes(1);
  f.client.dispose();
  await expect(pending).rejects.toThrow('connection closed');
  f.server.dispose();
});

test('forwards caller cancellation into a pending geometry mutation without an image reply', async () => {
  const f = fixture(true);
  let nativeCancelled = false;
  f.execute.mockImplementation(async (_command, cancellation) => {
    return await new Promise((resolve) => {
      cancellation?.subscribe(() => {
        nativeCancelled = true;
        resolve({ kind: 'failure', status: 'cancelled', message: 'Closed.' });
      });
    });
  });
  const cancellation = new CanvasCancellation();
  const pending = f.client.geometry.execute(
    { type: 'resize', document, size: { width: 1000, height: 1000 } },
    cancellation,
  );
  cancellation.cancel();
  expect(await pending).toMatchObject({ kind: 'failure', status: 'cancelled' });
  expect(nativeCancelled).toBe(true);
  expect(f.sent.at(-1)?.type).toBe('cancel');
  f.client.dispose();
  f.server.dispose();
});
