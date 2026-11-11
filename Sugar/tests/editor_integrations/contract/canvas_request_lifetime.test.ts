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
/** @jest-environment node */
/** Verify terminal outcomes at the shared request owner while replacing only native admission. */
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import type {
  CanvasMessage,
  CanvasContext,
  CanvasSourceContext,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { connectCanvasSurface, negotiatedSurface } from '../support/canvas_surface.js';

const session = 'request-lifetime-session';
const context: CanvasContext = {
  canvas: {
    id: 'native',
    label: 'Native',
    capabilities: {
      displayOutput: false,
      insertImageLayer: false,
      positionImageLayer: false,
    },
  },
  document: null,
};
const address = {
  sessionId: 'canvas:workflow',
  viewId: 'view:one',
  windowId: 'window:one',
  attachmentId: 'attachment:one',
  generation: 1,
} as const;

/** Admit messages synchronously and retain explicit host replies and deterministic deadline callbacks. */
function fixture(onSend: (message: CanvasMessage) => void = () => {}, settlesRequests = false) {
  const sent: CanvasMessage[] = [];
  const deadlines: { callback(): void; stop: ReturnType<typeof jest.fn<() => void>> }[] = [];
  const active = new Set<() => void>();
  const report = jest.fn<(error: unknown) => void>();
  const client = new CanvasClient(session, {
    send: (message) => {
      sent.push(message);
      onSend(message);
    },
    later: (_delay, callback) => {
      active.add(callback);
      const stop = jest.fn(() => {
        active.delete(callback);
      });
      deadlines.push({ callback, stop });
      return stop;
    },
    report,
    ...(settlesRequests ? { settlesRequests } : {}),
  });
  return {
    client,
    sent,
    deadlines,
    active,
    report,
    expire: (index: number): void => {
      const deadline = deadlines[index];
      if (!deadline || !active.has(deadline.callback))
        throw new Error('The requested deadline is not active.');
      deadline.callback();
    },
    refuse: (request: CanvasMessage | undefined): boolean => {
      if (!request) throw new Error('Missing refused request.');
      return client.receive({
        protocol: request.protocol,
        version: request.version,
        session: request.session,
        requestId: request.requestId,
        type: 'error.reply',
        failure: { status: 'unavailable', message: 'Native admission was refused.' },
      });
    },
  };
}

test('context timeout cancels its exact host request and permits a later context read', async () => {
  const f = fixture();
  const first = f.client.readContext();
  const rejection = expect(first).rejects.toThrow('timed out');
  f.expire(0);
  await rejection;
  expect(f.sent.map(({ type, requestId }) => ({ type, requestId }))).toEqual([
    { type: 'context.request', requestId: 1 },
    { type: 'cancel', requestId: 1 },
  ]);
  expect(f.deadlines[0]?.stop).toHaveBeenCalledTimes(1);
  expect(f.active.size).toBe(0);
  expect(
    f.client.receive({
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId: 1,
      type: 'error.reply',
      failure: { status: 'cancelled', message: 'Context read cancelled.' },
    }),
  ).toBe(false);
  const next = f.client.readContext();
  expect(f.sent.at(-1)).toMatchObject({ type: 'context.request', requestId: 2 });
  f.client.receive({ ...f.sent.at(-1), type: 'context.reply', context });
  await expect(next).resolves.toEqual(context);
  expect(f.client.readCanvas()?.id).toBe('native');
  expect(f.active.size).toBe(0);
  expect(f.report).not.toHaveBeenCalled();
  f.client.dispose();
});

test('context disposal sends one cancellation per outstanding request before refusing late replies', async () => {
  const f = fixture();
  const first = f.client.readContext();
  const second = f.client.readContext();
  const firstRejected = expect(first).rejects.toThrow('closed');
  const secondRejected = expect(second).rejects.toThrow('closed');
  f.client.dispose();
  f.client.dispose();
  await Promise.all([firstRejected, secondRejected]);
  expect(f.sent.map(({ type, requestId }) => ({ type, requestId }))).toEqual([
    { type: 'context.request', requestId: 1 },
    { type: 'context.request', requestId: 2 },
    { type: 'cancel', requestId: 1 },
    { type: 'cancel', requestId: 2 },
  ]);
  expect(f.active.size).toBe(0);
  for (const deadline of f.deadlines) expect(deadline.stop).toHaveBeenCalledTimes(1);
  expect(f.client.receive({ ...f.sent[0], type: 'context.reply', context })).toBe(false);
  await expect(f.client.readContext()).rejects.toThrow('closed');
  expect(f.sent).toHaveLength(4);
});

test('cancellation-lane refusal remains observable without replacing the context timeout', async () => {
  const refusal = new Error('Cancellation lane is unavailable.');
  const f = fixture((message) => {
    if (message.type === 'cancel') throw refusal;
  });
  const first = f.client.readContext();
  const rejected = expect(first).rejects.toThrow('timed out');
  f.expire(0);
  await rejected;
  expect(f.report).toHaveBeenCalledWith(refusal);
  expect(f.active.size).toBe(0);
  f.client.dispose();
});

test.each(['context', 'surface'] as const)(
  'asynchronous %s admission refusal settles once and retains later request progress',
  async (kind) => {
    const f = fixture();
    if (kind === 'surface') await connectCanvasSurface(f.client, f.sent);
    const requestIndex = f.sent.length;
    const pending =
      kind === 'context'
        ? f.client.readContext()
        : negotiatedSurface(f.client).execute({ type: 'surface.prepare', address, revision: 1 });
    const request = f.sent[requestIndex];
    const deadline = f.deadlines.at(-1);
    let outcomes = 0;
    const observed = pending.then(
      () => {
        outcomes++;
      },
      () => {
        outcomes++;
      },
    );
    const rejected = expect(pending).rejects.toThrow('Native admission was refused.');
    expect(outcomes).toBe(0);
    expect(f.refuse(request)).toBe(true);
    await rejected;
    await observed;
    expect(outcomes).toBe(1);
    expect(deadline?.stop).toHaveBeenCalledTimes(1);
    expect(f.refuse(request)).toBe(false);
    expect(f.active.size).toBe(0);
    expect(f.sent).toHaveLength(requestIndex + 1);
    const next = f.client.readContext();
    const nextRequest = f.sent.at(-1);
    if (!request || !nextRequest) throw new Error('Missing request sequence.');
    expect(nextRequest.requestId).toBe(request.requestId + 1);
    f.client.receive({ ...nextRequest, type: 'context.reply', context });
    await expect(next).resolves.toEqual(context);
    expect(f.active.size).toBe(0);
    expect(f.sent.filter(({ type }) => type === 'cancel')).toHaveLength(0);
    expect(f.report).not.toHaveBeenCalled();
    f.client.dispose();
  },
);

test('forwarded surface cancellation remains pending when cancellation admission is refused', async () => {
  const refusal = new Error('Cancellation credit is busy.');
  const f = fixture((message) => {
    if (message.type === 'cancel') throw refusal;
  });
  await connectCanvasSurface(f.client, f.sent);
  const cancellation = new CanvasCancellation();
  const pending = negotiatedSurface(f.client).execute(
    { type: 'surface.prepare', address, revision: 1 },
    cancellation,
  );
  let settled = false;
  const observed = pending.then(() => {
    settled = true;
  });
  cancellation.cancel();
  await Promise.resolve();
  expect(settled).toBe(false);
  expect(f.active.size).toBe(1);
  expect(f.report).toHaveBeenCalledWith(refusal);
  const request = f.sent.find((message) => message.type === 'surface.prepare');
  if (request?.type !== 'surface.prepare') throw new Error('Missing admitted preparation.');
  expect(
    f.client.receive({ ...request, type: 'surface.reply', status: 'ready', message: '' }),
  ).toBe(true);
  await expect(pending).resolves.toEqual({ status: 'ready', message: '' });
  await observed;
  expect(settled).toBe(true);
  expect(f.active.size).toBe(0);
  f.client.dispose();
});

test('a typed cancellation reply retires a bridge-held surface request without another request identity', async () => {
  const f = fixture();
  await connectCanvasSurface(f.client, f.sent);
  const cancellation = new CanvasCancellation();
  const pending = negotiatedSurface(f.client).execute(
    { type: 'surface.prepare', address, revision: 1 },
    cancellation,
  );
  const request = f.sent.at(-1);
  if (request?.type !== 'surface.prepare') throw new Error('Missing bridge-held request.');
  cancellation.cancel();
  expect(f.sent.at(-1)).toMatchObject({ type: 'cancel', requestId: request.requestId });
  const reply = { ...request, type: 'surface.reply', status: 'cancelled', message: '' };
  expect(f.client.receive(reply)).toBe(true);
  await expect(pending).resolves.toEqual({ status: 'cancelled', message: '' });
  expect(f.client.receive(reply)).toBe(false);
  expect(f.deadlines.at(-1)?.stop).toHaveBeenCalledTimes(1);
  expect(f.active.size).toBe(0);
  expect(f.report).not.toHaveBeenCalled();
  f.client.dispose();
});

describe('a transport that settles every request itself', () => {
  test('waits for context discovery and surface replies without a deadline, however slow the host', async () => {
    const f = fixture(() => {}, true);
    await connectCanvasSurface(f.client, f.sent);
    // Discovery itself never registered a deadline.
    expect(f.deadlines).toHaveLength(0);
    const requestIndex = f.sent.length;
    const outcome = negotiatedSurface(f.client).execute({
      type: 'surface.prepare',
      address,
      revision: 1,
    });
    let settled = false;
    void outcome.then(
      () => {
        settled = true;
      },
      () => {
        settled = true;
      },
    );
    await Promise.resolve();
    expect(f.deadlines).toHaveLength(0);
    expect(f.active.size).toBe(0);
    expect(settled).toBe(false);
    const request = f.sent[requestIndex];
    if (request?.type !== 'surface.prepare') throw new Error('Missing surface request.');
    // A host that finally answers is accepted: waiting was the whole of the policy.
    f.client.receive({
      protocol: request.protocol,
      version: request.version,
      session: request.session,
      requestId: request.requestId,
      type: 'surface.reply',
      address,
      revision: 1,
      status: 'ready',
      message: '',
    });
    await expect(outcome).resolves.toMatchObject({ status: 'ready' });
    expect(f.report).not.toHaveBeenCalled();
    f.client.dispose();
  });

  test('ends a waiting surface request only on observed loss of the connection', async () => {
    const f = fixture(() => {}, true);
    await connectCanvasSurface(f.client, f.sent);
    const outcome = negotiatedSurface(f.client).execute({
      type: 'surface.prepare',
      address,
      revision: 1,
    });
    const rejected = expect(outcome).rejects.toThrow('closed');
    await Promise.resolve();
    expect(f.deadlines).toHaveLength(0);
    f.client.dispose();
    await rejected;
    expect(f.sent.at(-1)).toMatchObject({ type: 'cancel' });
  });

  test('waits behind a slow surface request for source and insertion replies without a deadline', async () => {
    const f = fixture(() => {}, true);
    // The native lane admits one request at a time, so a source query queued behind a surface
    // request the host is still preparing would otherwise time out while only waiting its turn.
    const context = f.client.sources.execute({ type: 'context' });
    const insertion = f.client.insertImage({
      image: new Uint8Array([1]),
      format: 'png',
      name: 'x',
      position: 'center',
    } as unknown as Parameters<CanvasClient['insertImage']>[0]);
    await Promise.resolve();
    expect(f.deadlines).toHaveLength(0);
    const [sourceRequest, insertRequest] = f.sent;
    if (sourceRequest?.type !== 'source.request' || insertRequest?.type !== 'image.insert')
      throw new Error('Missing queued requests.');
    const reply = { protocol: sourceRequest.protocol, version: sourceRequest.version, session };
    const sourceContext: CanvasSourceContext = {
      observesChanges: true,
      kind: 'context',
      supported: true,
      document: { id: 'doc', geometryRevision: 'geometry', width: 10, height: 20 },
      revision: '1',
      targets: ['composition'],
      layers: [],
      currentLayerId: null,
    };
    const refused = expect(insertion).rejects.toThrow('The canvas is not ready');
    expect(
      f.client.receive({
        ...reply,
        requestId: sourceRequest.requestId,
        type: 'source.reply',
        result: sourceContext,
      }),
    ).toBe(true);
    f.client.receive({
      ...reply,
      requestId: insertRequest.requestId,
      type: 'error.reply',
      failure: { status: 'unavailable', message: 'The canvas is not ready' },
    });
    await expect(context).resolves.toEqual(sourceContext);
    await refused;
    expect(f.report).not.toHaveBeenCalled();
    f.client.dispose();
  });
});
