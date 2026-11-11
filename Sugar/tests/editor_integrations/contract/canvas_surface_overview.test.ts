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
/** Qualify native overview grids through shared vectors and the real canvas session. */
import { readFileSync } from 'node:fs';
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import { isCanvasSurfaceOverviewRequest } from '../../../integrations/shared/CanvasSurfaceValidation.generated.js';
import { CANVAS_SURFACE_MESSAGE_BYTES } from '../../../integrations/shared/CanvasMessageSize.js';
import {
  CANVAS_SURFACE_OVERVIEW_CELLS,
  canvasSurfaceOverviewMessageFits,
  fittingCanvasSurfaceOverviewCells,
  validCanvasSurfaceOverview,
} from '../../../integrations/shared/CanvasSurfaceOverview.js';
import {
  snapshotCanvasSurfacePresentation,
  validCanvasSurfacePresentation,
} from '../../../integrations/shared/CanvasSurfacePreview.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfaceOverviewCell,
  CanvasSurfacePresentationChange,
  CanvasSurfaceResult,
} from '../../../integrations/contracts/CanvasSurface.js';
import {
  connectCanvasSurface,
  negotiatedSurface,
  selectedPngCapabilities,
} from '../support/canvas_surface.js';

interface Vector {
  readonly name: string;
  readonly value: Record<string, unknown>;
}
const vectors: {
  messageBytes: number;
  maxCells: number;
  maxItemIdCodePoints: number;
  validRequests: Vector[];
  invalidRequests: Vector[];
  semanticallyInvalidRequests: Vector[];
  oversizedRequests: Vector[];
  presentation: { valid: Vector[]; invalid: Vector[]; unpresentable: Vector[] };
} = JSON.parse(
  readFileSync(new URL('../support/canvas_surface_overview_vectors.json', import.meta.url), 'utf8'),
);
const session = 'surface-overview-session';
const address = {
  sessionId: 'canvas:workflow',
  viewId: 'view:one',
  windowId: 'window:one',
  attachmentId: 'attachment:one',
  generation: 1,
} as const;
const overviewCapabilities: CanvasSurfaceCapabilities = {
  ...selectedPngCapabilities,
  overview: true,
};

/** Describe one painted cell at a distinct horizontal offset. */
function cell(index: number): CanvasSurfaceOverviewCell {
  return {
    itemId: `result:${index}`,
    file: { filename: `ComfyUI_${index}.png`, subfolder: '', type: 'output' },
    rect: { x: index * 100, y: 0, width: 90, height: 60 },
  };
}

/** Read one presentation vector's change after confirming it is a wire message. */
function change(vector: Vector): CanvasSurfacePresentationChange {
  const message = vector.value;
  if (!isCanvasMessage(message) || message.type !== 'surface.presentation.changed')
    throw new Error(`Invalid presentation vector: ${vector.name}`);
  return message.change;
}

test('pins the shared overview bounds that the native host mirrors', () => {
  expect(CANVAS_SURFACE_MESSAGE_BYTES).toBe(vectors.messageBytes);
  expect(CANVAS_SURFACE_OVERVIEW_CELLS).toBe(vectors.maxCells);
  expect(vectors.maxItemIdCodePoints).toBe(1024);
});

test.each(vectors.validRequests)('admits overview vector: $name', ({ value }) => {
  expect(isCanvasMessage(value)).toBe(true);
  expect(isCanvasSurfaceOverviewRequest(value)).toBe(true);
  if (!isCanvasSurfaceOverviewRequest(value)) throw new Error('Unreachable.');
  expect(validCanvasSurfaceOverview(value)).toBe(true);
  expect(canvasSurfaceOverviewMessageFits(value)).toBe(true);
});

test.each(vectors.invalidRequests)('refuses malformed overview vector: $name', ({ value }) => {
  expect(isCanvasMessage(value)).toBe(false);
});

test.each(vectors.semanticallyInvalidRequests)(
  'refuses schema-valid overview vector semantically: $name',
  ({ value }) => {
    expect(isCanvasSurfaceOverviewRequest(value)).toBe(true);
    if (!isCanvasSurfaceOverviewRequest(value)) throw new Error('Unreachable.');
    expect(validCanvasSurfaceOverview(value)).toBe(false);
  },
);

test.each(vectors.oversizedRequests)('refuses an oversized overview vector: $name', ({ value }) => {
  expect(isCanvasMessage(value)).toBe(true);
  expect(canvasSurfaceOverviewMessageFits(value)).toBe(false);
});

test('refuses non-finite rect coordinates that JSON vectors cannot carry', () => {
  const valid = vectors.validRequests[0]?.value;
  if (!isCanvasSurfaceOverviewRequest(valid)) throw new Error('Missing vector.');
  const [first] = valid.cells;
  if (!first) throw new Error('Missing cell.');
  for (const number of [Number.NaN, Infinity, -Infinity])
    for (const key of ['x', 'y', 'width', 'height'] as const)
      expect(
        isCanvasMessage({
          ...valid,
          cells: [{ ...first, rect: { ...first.rect, [key]: number } }],
        }),
      ).toBe(false);
});

test('keeps the overview display variant closed and unpresentable as preview pixels', () => {
  for (const vector of vectors.presentation.valid) {
    const observed = change(vector);
    expect(validCanvasSurfacePresentation(observed)).toBe(true);
    const snapshot = snapshotCanvasSurfacePresentation(observed);
    expect(Object.isFrozen(snapshot.displayed)).toBe(true);
    expect(snapshot.displayed).toEqual({ type: 'overview', selectionRevision: 7 });
  }
  for (const vector of vectors.presentation.invalid)
    expect(isCanvasMessage(vector.value)).toBe(false);
  for (const vector of vectors.presentation.unpresentable)
    expect(validCanvasSurfacePresentation(change(vector))).toBe(false);
});

test('trims trailing cells until the request fits under any session envelope', () => {
  const wide = Array.from({ length: 8 }, (_, index) => ({
    ...cell(index),
    file: {
      filename: `${'a'.repeat(4092)}.png`,
      subfolder: 'b'.repeat(4096),
      type: 'output' as const,
    },
  }));
  const fitted = fittingCanvasSurfaceOverviewCells(address, wide);
  expect(fitted.length).toBeGreaterThan(0);
  expect(fitted.length).toBeLessThan(8);
  expect(fitted).toEqual(wide.slice(0, fitted.length));
  const message = {
    protocol: 'sugarcubes.canvas',
    version: 3,
    session: 'x'.repeat(80),
    requestId: 2147483647,
    type: 'surface.overview',
    address,
    revision: Number.MAX_SAFE_INTEGER,
  };
  expect(canvasSurfaceOverviewMessageFits({ ...message, cells: fitted })).toBe(true);
  expect(
    canvasSurfaceOverviewMessageFits({ ...message, cells: wide.slice(0, fitted.length + 1) }),
  ).toBe(false);
  const small = Array.from({ length: 9 }, (_, index) => cell(index));
  expect(fittingCanvasSurfaceOverviewCells(address, small)).toEqual(small.slice(0, 8));
});

/** Exercise real client/server correlation while faking only the native surface. */
function fixture(capabilities: CanvasSurfaceCapabilities) {
  const sent: CanvasMessage[] = [];
  const errors: unknown[] = [];
  const execute = jest.fn(
    async (command: CanvasSurfaceCommand): Promise<CanvasSurfaceResult> => ({
      status: command.type === 'surface.prepare' ? 'ready' : 'applied',
      message: '',
    }),
  );
  const server = new CanvasServer(session, {
    surface: { capabilities, execute },
    context: async () => ({
      canvas: {
        id: 'native',
        label: 'Native',
        capabilities: { displayOutput: true, insertImageLayer: false, positionImageLayer: false },
      },
      document: null,
    }),
    insert: async () => ({ status: 'unsupported', message: 'No layers.' }),
    send: (message) => client.receive(message),
    report: (error) => errors.push(error),
  });
  const client = new CanvasClient(session, {
    send: (message) => {
      sent.push(message);
      server.receive(message);
    },
    later: () => () => {},
    report: (error) => errors.push(error),
  });
  return { client, server, execute, sent, errors };
}

/** Drain the shared session's asynchronous context and dispatch continuations. */
async function settle(): Promise<void> {
  for (let turn = 0; turn < 8; turn++) await Promise.resolve();
}

test.each([true, false])(
  'carries an immutable overview only to a host declaring overview: %s',
  async (overview) => {
    const f = fixture({ ...selectedPngCapabilities, overview });
    try {
      await f.client.readContext();
      const request = {
        type: 'surface.overview',
        address,
        revision: 4,
        cells: [cell(0), cell(1)],
      } as const;
      const result = await negotiatedSurface(f.client).execute(request);
      expect(result.status).toBe(overview ? 'applied' : 'unavailable');
      expect(f.sent.some((message) => message.type === 'surface.overview')).toBe(overview);
      // A client that ignores negotiation is refused by the receiving surface owner as well.
      f.server.receive({
        protocol: 'sugarcubes.canvas',
        version: 3,
        session,
        requestId: 90,
        ...request,
      });
      await settle();
      const calls = f.execute.mock.calls.map(([command]) => command);
      expect(calls).toEqual(overview ? [request, request] : []);
      for (const command of calls) {
        if (command.type !== 'surface.overview') throw new Error('Missing overview.');
        expect(Object.isFrozen(command.cells)).toBe(true);
        expect(command.cells.every((value) => Object.isFrozen(value.rect))).toBe(true);
        expect(command.cells.every((value) => Object.isFrozen(value.file))).toBe(true);
      }
      expect(f.errors).toEqual([]);
    } finally {
      f.client.dispose();
      f.server.dispose();
    }
  },
);

test('refuses duplicate results before transport and at the receiving surface owner', async () => {
  const f = fixture(overviewCapabilities);
  try {
    await f.client.readContext();
    const duplicate = {
      type: 'surface.overview',
      address,
      revision: 4,
      cells: [cell(0), { ...cell(1), itemId: 'result:0' }],
    } as const;
    const sent = f.sent.length;
    expect((await negotiatedSurface(f.client).execute(duplicate)).status).toBe('error');
    expect(f.sent).toHaveLength(sent);
    const replies: CanvasMessage[] = [];
    const server = new CanvasServer(session, {
      surface: { capabilities: overviewCapabilities, execute: f.execute },
      context: async () => ({
        canvas: {
          id: 'native',
          label: 'Native',
          capabilities: { displayOutput: true, insertImageLayer: false, positionImageLayer: false },
        },
        document: null,
      }),
      insert: async () => ({ status: 'unsupported', message: 'No layers.' }),
      send: (message) => replies.push(message),
      report: (error) => f.errors.push(error),
    });
    server.receive({
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId: 91,
      ...duplicate,
    });
    await settle();
    expect(replies).toEqual([
      expect.objectContaining({ type: 'surface.reply', requestId: 91, status: 'unavailable' }),
    ]);
    expect(f.execute).not.toHaveBeenCalled();
    server.dispose();
  } finally {
    f.client.dispose();
    f.server.dispose();
  }
});

test('never transports an overview over the native message ceiling', async () => {
  const sent: CanvasMessage[] = [];
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: () => {},
  });
  await connectCanvasSurface(client, sent, overviewCapabilities);
  const oversized = vectors.oversizedRequests[0]?.value;
  if (!isCanvasSurfaceOverviewRequest(oversized)) throw new Error('Missing vector.');
  const before = sent.length;
  await expect(
    negotiatedSurface(client).execute({
      type: 'surface.overview',
      address: oversized.address,
      revision: oversized.revision,
      cells: oversized.cells,
    }),
  ).rejects.toThrow('exceeds the native transport bounds');
  expect(sent).toHaveLength(before);
  client.dispose();
});
