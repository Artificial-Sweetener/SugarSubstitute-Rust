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
/** Qualify negotiated viewport commands through the existing session and cancellation owner. */
import { readFileSync } from 'node:fs';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import {
  isCanvasSurfaceViewportRequest,
  isCanvasSurfaceViewportReply,
} from '../../../integrations/shared/CanvasSurfaceValidation.generated.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfaceResult,
  CanvasSurfaceViewportCommand,
} from '../../../integrations/contracts/CanvasSurface.js';
import {
  connectCanvasSurface,
  negotiatedSurface,
  selectedPngCapabilities,
} from '../support/canvas_surface.js';

const session = 'viewport-contract-session';
const address = {
  sessionId: 'canvas:workflow',
  viewId: 'view:one',
  windowId: 'window:one',
  attachmentId: 'attachment:one',
  generation: 1,
} as const;
const capabilities: CanvasSurfaceCapabilities = {
  ...selectedPngCapabilities,
  viewportCommands: ['fit', 'native', 'zoom', 'pan'],
  viewportState: true,
};
const viewport = {
  mode: 'fit',
  physicalScale: 0.5,
  dimensions: { width: 640, height: 480 },
} as const;
const vectors: {
  validRequests: unknown[];
  validReplies: unknown[];
  invalidRequests: unknown[];
  invalidReplies: unknown[];
} = JSON.parse(
  readFileSync(new URL('../support/canvas_surface_viewport_vectors.json', import.meta.url), 'utf8'),
);

/** Keep transport and deadline effects observable without introducing real timers. */
async function fixture(declared = capabilities) {
  const sent: CanvasMessage[] = [];
  const deadlines: (() => void)[] = [];
  const errors: unknown[] = [];
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: (_milliseconds, callback) => {
      deadlines.push(callback);
      return () => {};
    },
    report: (error) => errors.push(error),
  });
  await connectCanvasSurface(client, sent, declared);
  sent.length = 0;
  return { client, sent, deadlines, errors };
}

/** Submit a camera intent through the same surface host used by legacy presentation. */
function request(
  f: Awaited<ReturnType<typeof fixture>>,
  command: CanvasSurfaceViewportCommand = { type: 'read' },
  cancellation?: CanvasCancellation,
) {
  return negotiatedSurface(f.client).execute(
    { type: 'surface.viewport.request', address, revision: 3, command },
    cancellation,
  );
}

/** Respond to the outstanding request without guessing the shared request sequence. */
function reply(f: Awaited<ReturnType<typeof fixture>>) {
  const last = f.sent.at(-1);
  if (last?.type !== 'surface.viewport.request') throw new Error('Missing viewport request.');
  return {
    protocol: last.protocol,
    version: last.version,
    session: last.session,
    requestId: last.requestId,
    type: 'surface.viewport.reply',
    address: { ...last.address },
    revision: last.revision,
    status: 'applied',
    message: '',
    viewport: { ...viewport, dimensions: { ...viewport.dimensions } },
  } as const;
}

test('accepts every shared viewport vector and rejects malformed bounded shapes', () => {
  for (const value of vectors.validRequests) {
    expect(isCanvasSurfaceViewportRequest(value)).toBe(true);
    expect(isCanvasMessage(value)).toBe(true);
  }
  for (const value of vectors.validReplies) {
    expect(isCanvasSurfaceViewportReply(value)).toBe(true);
    expect(isCanvasMessage(value)).toBe(true);
  }
  for (const value of [...vectors.invalidRequests, ...vectors.invalidReplies])
    expect(isCanvasMessage(value)).toBe(false);
  const valid = vectors.validRequests[0];
  if (!isCanvasSurfaceViewportRequest(valid)) throw new Error('Missing vector.');
  for (const factor of [NaN, Infinity, -Infinity])
    expect(isCanvasMessage({ ...valid, command: { type: 'zoom', factor } })).toBe(false);
});

test.each(vectors.validRequests)(
  'uses the existing request sequence for a viewport command %#',
  async (value) => {
    if (!isCanvasSurfaceViewportRequest(value)) throw new Error('Invalid viewport fixture.');
    const f = await fixture();
    try {
      const pending = request(f, value.command);
      expect(f.sent).toHaveLength(1);
      expect(f.sent[0]?.requestId).toBe(3);
      expect(f.client.receive(reply(f))).toBe(true);
      await expect(pending).resolves.toEqual({ status: 'applied', message: '', viewport });
      expect(f.client.receive(reply(f))).toBe(false);
    } finally {
      f.client.dispose();
    }
  },
);

test.each([
  selectedPngCapabilities,
  { ...capabilities, viewportState: false },
  { ...capabilities, viewportCommands: [] },
])('refuses unadvertised commands locally without occupying transport %#', async (declared) => {
  const f = await fixture(declared);
  try {
    await expect(request(f, { type: 'fit' })).resolves.toMatchObject({ status: 'unavailable' });
    expect(f.sent).toEqual([]);
  } finally {
    f.client.dispose();
  }
});

test('permits state reads independently of individual command support', async () => {
  const f = await fixture({ ...capabilities, viewportCommands: [] });
  const pending = request(f);
  f.client.receive(reply(f));
  await expect(pending).resolves.toMatchObject({ viewport });
  f.client.dispose();
});

test('retains immutable native state before a transport caller can mutate its reply', async () => {
  const f = await fixture();
  const pending = request(f);
  const value = reply(f);
  f.client.receive(value);
  Object.assign(value.viewport, { physicalScale: 4 });
  Object.assign(value.viewport.dimensions, { width: 1 });
  Object.assign(value.address, { viewId: 'view:wrong' });
  const result = await pending;
  expect(result).toEqual({ status: 'applied', message: '', viewport });
  if (!('viewport' in result)) throw new Error('Missing native observation.');
  expect(Object.isFrozen(result.viewport)).toBe(true);
  expect(Object.isFrozen(result.viewport.dimensions)).toBe(true);
  f.client.dispose();
});

test.each(['stale', 'cancelled', 'unavailable', 'error'] as const)(
  'accepts existing %s surface replies for viewport requests',
  async (status) => {
    const f = await fixture();
    const pending = request(f);
    const { viewport: _state, ...value } = reply(f);
    expect(f.client.receive({ ...value, type: 'surface.reply', status })).toBe(true);
    await expect(pending).resolves.toEqual({ status, message: '' });
    f.client.dispose();
  },
);

test('ignores status-only success and waits for the authoritative camera reply', async () => {
  const f = await fixture();
  const pending = request(f);
  const { viewport: _state, ...value } = reply(f);
  expect(f.client.receive({ ...value, type: 'surface.reply' })).toBe(false);
  expect(f.client.receive(reply(f))).toBe(true);
  await expect(pending).resolves.toMatchObject({ viewport });
  f.client.dispose();
});

test.each(['address', 'revision'] as const)(
  'rejects a viewport reply with foreign %s',
  async (field) => {
    const f = await fixture();
    const pending = request(f);
    const value = reply(f);
    expect(
      f.client.receive(
        field === 'address'
          ? { ...value, address: { ...address, generation: 2 } }
          : { ...value, revision: 4 },
      ),
    ).toBe(true);
    await expect(pending).rejects.toThrow('does not match');
    f.client.dispose();
  },
);

test('waits for existing cancellation acknowledgement and ignores its late viewport result', async () => {
  const f = await fixture();
  const cancellation = new CanvasCancellation();
  const pending = request(f, { type: 'zoom', factor: 1.25 }, cancellation);
  const value = reply(f);
  cancellation.cancel();
  expect(f.sent.map(({ type }) => type)).toEqual(['surface.viewport.request', 'cancel']);
  expect(f.sent[0]?.requestId).toBe(f.sent[1]?.requestId);
  const { viewport: _state, ...envelope } = value;
  f.client.receive({ ...envelope, type: 'surface.reply', status: 'cancelled' });
  await expect(pending).resolves.toMatchObject({ status: 'cancelled' });
  expect(f.client.receive(value)).toBe(false);
  f.client.dispose();
});

test('shares capable/limited host dispatch and duplicate suppression', async () => {
  for (const capable of [true, false]) {
    const calls: CanvasSurfaceCommand[] = [];
    const sent: CanvasMessage[] = [];
    const errors: unknown[] = [];
    const server = new CanvasServer(session, {
      surface: {
        capabilities: capable ? capabilities : selectedPngCapabilities,
        execute: async (command): Promise<CanvasSurfaceResult> => {
          calls.push(command);
          return command.type === 'surface.viewport.request'
            ? { status: 'applied', message: '', viewport }
            : { status: command.type === 'surface.prepare' ? 'ready' : 'applied', message: '' };
        },
      },
      context: async () => ({
        canvas: {
          id: 'native',
          label: 'Native',
          capabilities: { displayOutput: true, insertImageLayer: false, positionImageLayer: false },
        },
        document: null,
      }),
      insert: async () => ({ status: 'unsupported', message: '' }),
      send: (message) => {
        sent.push(message);
        client.receive(message);
      },
      report: (error) => errors.push(error),
    });
    const requests: CanvasMessage[] = [];
    const client = new CanvasClient(session, {
      send: (message) => {
        requests.push(message);
        server.receive(message);
      },
      later: () => () => {},
      report: (error) => errors.push(error),
    });
    try {
      await client.readContext();
      const command = {
        type: 'surface.viewport.request',
        address,
        revision: 3,
        command: { type: 'fit' },
      } as const;
      const result = await negotiatedSurface(client).execute(command);
      expect(result.status).toBe(capable ? 'applied' : 'unavailable');
      expect(calls).toHaveLength(capable ? 1 : 0);
      if (capable) {
        const original = requests.at(-1);
        if (!original) throw new Error('Missing original request.');
        server.receive(original);
        expect(calls).toHaveLength(1);
        expect(sent.at(-1)?.type).toBe('surface.viewport.reply');
      } else {
        server.receive({
          protocol: 'sugarcubes.canvas',
          version: 3,
          session,
          requestId: 3,
          ...command,
        });
        await Promise.resolve();
        await Promise.resolve();
        expect(calls).toHaveLength(0);
        expect(sent.at(-1)).toMatchObject({ type: 'surface.reply', status: 'unavailable' });
      }
      expect(errors).toEqual([]);
    } finally {
      client.dispose();
      server.dispose();
    }
  }
});

test('keeps an already cancelled viewport request local and reuses normal deadline cancellation', async () => {
  const f = await fixture();
  const cancellation = new CanvasCancellation();
  cancellation.cancel();
  await expect(request(f, { type: 'native' }, cancellation)).resolves.toMatchObject({
    status: 'cancelled',
  });
  expect(f.sent).toEqual([]);
  const pending = request(f);
  const deadline = f.deadlines.at(-1);
  if (!deadline) throw new Error('Missing request deadline.');
  deadline();
  await expect(pending).rejects.toThrow('timed out');
  expect(f.sent.map(({ type }) => type)).toEqual(['surface.viewport.request', 'cancel']);
  expect(f.sent[0]?.requestId).toBe(f.sent[1]?.requestId);
  f.client.dispose();
});

test('preserves generic failure replies and refuses viewport results for legacy operations', async () => {
  const f = await fixture();
  const pending = request(f);
  const value = reply(f);
  expect(
    f.client.receive({
      protocol: value.protocol,
      version: value.version,
      session: value.session,
      requestId: value.requestId,
      type: 'error.reply',
      failure: { status: 'cancelled', message: 'The original camera request was cancelled.' },
    }),
  ).toBe(true);
  await expect(pending).rejects.toThrow('original camera request');
  const prepared = negotiatedSurface(f.client).execute({
    type: 'surface.prepare',
    address,
    revision: 4,
  });
  const original = f.sent.at(-1);
  if (original?.type !== 'surface.prepare') throw new Error('Missing legacy request.');
  expect(f.client.receive({ ...value, requestId: original.requestId, revision: 4 })).toBe(false);
  const { viewport: _state, ...envelope } = value;
  f.client.receive({
    ...envelope,
    requestId: original.requestId,
    revision: 4,
    type: 'surface.reply',
    status: 'ready',
  });
  await expect(prepared).resolves.toEqual({ status: 'ready', message: '' });
  f.client.dispose();
});

test('disposal cancels the original viewport identity and rejects late state', async () => {
  const f = await fixture();
  const pending = request(f);
  const value = reply(f);
  f.client.dispose();
  await expect(pending).rejects.toThrow('closed');
  expect(f.sent.map(({ type }) => type)).toEqual(['surface.viewport.request', 'cancel']);
  expect(f.sent[0]?.requestId).toBe(f.sent[1]?.requestId);
  expect(f.client.receive(value)).toBe(false);
});

test('server cancellation retains the original scope and never publishes late state', async () => {
  const sent: CanvasMessage[] = [];
  const errors: unknown[] = [];
  let finish: ((value: CanvasSurfaceResult) => void) | undefined;
  const server = new CanvasServer(session, {
    surface: {
      capabilities,
      execute: (_command, cancellation) =>
        new Promise((resolve) => {
          finish = resolve;
          cancellation?.subscribe(() => {
            resolve({ status: 'applied', message: '', viewport });
          });
        }),
    },
    context: async () => ({
      canvas: {
        id: 'native',
        label: 'Native',
        capabilities: { displayOutput: true, insertImageLayer: false, positionImageLayer: false },
      },
      document: null,
    }),
    insert: async () => ({ status: 'unsupported', message: '' }),
    send: (value) => sent.push(value),
    report: (error) => errors.push(error),
  });
  const original = {
    protocol: 'sugarcubes.canvas',
    version: 3,
    session,
    requestId: 1,
    type: 'surface.viewport.request',
    address,
    revision: 1,
    command: { type: 'zoom', factor: 1.25 },
  } as const;
  try {
    expect(server.receive(original)).toBe(true);
    await Promise.resolve();
    if (!finish) throw new Error('Native operation was not dispatched.');
    expect(
      server.receive({
        protocol: original.protocol,
        version: original.version,
        session,
        requestId: 1,
        type: 'cancel',
      }),
    ).toBe(true);
    await Promise.resolve();
    await Promise.resolve();
    expect(sent).toEqual([
      {
        protocol: original.protocol,
        version: original.version,
        session,
        requestId: 1,
        type: 'surface.reply',
        address,
        revision: 1,
        status: 'cancelled',
        message: 'Canvas surface request cancelled.',
      },
    ]);
    finish({ status: 'applied', message: '', viewport });
    await Promise.resolve();
    expect(sent).toHaveLength(1);
    expect(errors).toEqual([]);
  } finally {
    server.dispose();
  }
});
