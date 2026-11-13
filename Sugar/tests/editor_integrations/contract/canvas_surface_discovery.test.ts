/** Prove optional surface discovery cannot replace session correlation or document capability ownership. */
import { readFileSync } from 'node:fs';
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { EMPTY_CANVAS_SURFACE_CAPABILITIES } from '../../../integrations/shared/CanvasSurfaceDiscovery.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import type {
  CanvasContext,
  CanvasMessage,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasSurfaceCapabilitiesReply } from '../../../integrations/contracts/CanvasSurfaceProtocol.generated.js';
import {
  connectCanvasSurface,
  negotiatedSurface,
  selectedPngCapabilities,
} from '../support/canvas_surface.js';

const session = 'surface-discovery-session';
const address = {
  sessionId: 'canvas:workflow',
  viewId: 'view:one',
  windowId: 'window:one',
  attachmentId: 'attachment:one',
  generation: 1,
} as const;
const context: CanvasContext = {
  canvas: {
    id: 'native',
    label: 'Native',
    capabilities: {
      displayOutput: true,
      insertImageLayer: false,
      positionImageLayer: false,
    },
  },
  document: null,
};

/** Retain messages and injected deadlines without real timers. */
function fixture() {
  const sent: CanvasMessage[] = [];
  const report = jest.fn<(error: unknown) => void>();
  const stop = jest.fn();
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => stop,
    report,
  });
  return { client, sent, report, stop };
}

/** Deliver exactly the outstanding context identity. */
function contextReply(f: ReturnType<typeof fixture>, index: number, value = context): void {
  expect(f.client.receive({ ...f.sent[index], type: 'context.reply', context: value })).toBe(true);
}

/** Retain the public discovery envelope without carrying its request-only schema field. */
function capabilitiesReply(
  request: CanvasMessage | undefined,
  status: CanvasSurfaceCapabilitiesReply['status'] = 'available',
): CanvasSurfaceCapabilitiesReply {
  if (request?.type !== 'surface.capabilities.request')
    throw new Error('Missing discovery request.');
  return {
    protocol: request.protocol,
    version: request.version,
    session: request.session,
    requestId: request.requestId,
    type: 'surface.capabilities.reply',
    status,
    capabilities:
      status === 'available' ? selectedPngCapabilities : EMPTY_CANVAS_SURFACE_CAPABILITIES,
    message: '',
  };
}

test.each(['photoshop', 'gimp'])(
  'never sends optional discovery to %s without display output',
  async (id) => {
    const f = fixture();
    const pending = f.client.readContext();
    contextReply(f, 0, {
      ...context,
      canvas: {
        ...context.canvas,
        id,
        capabilities: {
          displayOutput: false,
          insertImageLayer: true,
          positionImageLayer: true,
          geometry: true,
        },
      },
    });
    await pending;
    expect(f.sent.map(({ type }) => type)).toEqual(['context.request']);
    expect(f.client.surface).toBeNull();
    expect(f.client.readCanvas()?.capabilities.insertImageLayer).toBe(true);
    f.client.dispose();
  },
);

test.each(['unsupported', 'unavailable', 'error'] as const)(
  'keeps a %s surface explicitly absent',
  async (status) => {
    const f = fixture();
    const pending = f.client.readContext();
    contextReply(f, 0);
    await Promise.resolve();
    expect(f.client.receive(capabilitiesReply(f.sent[1], status))).toBe(true);
    await pending;
    expect(f.client.surface).toBeNull();
    expect(f.report).toHaveBeenCalledTimes(status === 'error' ? 1 : 0);
    f.client.dispose();
  },
);

test('rejects executable declarations attached to an unavailable discovery reply', async () => {
  const f = fixture();
  const pending = f.client.readContext();
  contextReply(f, 0);
  await Promise.resolve();
  const reply = { ...capabilitiesReply(f.sent[1]), status: 'unsupported' };
  f.client.receive(reply);
  await pending;
  expect(f.client.surface).toBeNull();
  expect(f.report).toHaveBeenCalledWith(
    expect.objectContaining({ message: expect.stringContaining('Unavailable') }),
  );
  f.client.dispose();
});

test('ignores an older context before the newest context has even replied', async () => {
  const f = fixture();
  const first = f.client.readContext();
  const second = f.client.readContext();
  contextReply(f, 0);
  await first;
  expect(f.client.readCanvas()).toBeNull();
  expect(f.sent.map(({ type }) => type)).toEqual(['context.request', 'context.request']);
  contextReply(f, 1, {
    ...context,
    canvas: {
      ...context.canvas,
      capabilities: { ...context.canvas.capabilities, displayOutput: false },
    },
  });
  await second;
  expect(f.client.surface).toBeNull();
  f.client.dispose();
});

test('an old discovery cannot publish while a newer context read is pending', async () => {
  const f = fixture();
  const first = f.client.readContext();
  contextReply(f, 0);
  await Promise.resolve();
  const second = f.client.readContext();
  f.client.receive(capabilitiesReply(f.sent[1]));
  await first;
  expect(f.client.surface).toBeNull();
  contextReply(f, 2, { ...context, canvas: { ...context.canvas, id: 'new-host' } });
  await Promise.resolve();
  f.client.receive(capabilitiesReply(f.sent[3]));
  await second;
  expect(negotiatedSurface(f.client).capabilities.imageFormats).toEqual(['png']);
  expect(f.client.readCanvas()?.id).toBe('new-host');
  f.client.dispose();
});

test('snapshots host declarations and permits exact attachment teardown after context withdrawal', async () => {
  const f = fixture();
  await connectCanvasSurface(f.client, f.sent);
  const surface = negotiatedSurface(f.client);
  expect(Object.isFrozen(surface.capabilities.imageFormats)).toBe(true);
  const next = f.client.readContext();
  expect(f.client.surface).toBeNull();
  await expect(
    surface.execute({ type: 'surface.prepare', address, revision: 1 }),
  ).resolves.toMatchObject({ status: 'unavailable' });
  const release = surface.execute({ type: 'surface.release', address, revision: 2 });
  const request = f.sent.at(-1);
  if (request?.type !== 'surface.release') throw new Error('Missing release request.');
  f.client.receive({ ...request, type: 'surface.reply', status: 'applied', message: '' });
  await expect(release).resolves.toMatchObject({ status: 'applied' });
  contextReply(f, 2, {
    ...context,
    canvas: {
      ...context.canvas,
      capabilities: { ...context.canvas.capabilities, displayOutput: false },
    },
  });
  await next;
  f.client.dispose();
});

test('uses existing replay protection for a limited host discovery without layer insertion', async () => {
  const replies: CanvasMessage[] = [];
  const insert = jest.fn<ConstructorParameters<typeof CanvasServer>[1]['insert']>();
  const server = new CanvasServer(session, {
    context: async () => context,
    insert,
    send: (message) => replies.push(message),
    report: () => {},
  });
  const request = {
    protocol: 'sugarcubes.canvas',
    version: 3,
    session,
    requestId: 1,
    type: 'surface.capabilities.request',
    schema: 2,
  } as const;
  server.receive(request);
  await Promise.resolve();
  await Promise.resolve();
  expect(replies[0]).toMatchObject({
    type: 'surface.capabilities.reply',
    status: 'unsupported',
    capabilities: EMPTY_CANVAS_SURFACE_CAPABILITIES,
  });
  server.receive(request);
  expect(replies).toHaveLength(2);
  expect(insert).not.toHaveBeenCalled();
  expect(server.receive({ ...request, requestId: 2, schema: 4 })).toBe(true);
  await Promise.resolve();
  await Promise.resolve();
  expect(replies[2]).toMatchObject({
    type: 'surface.capabilities.reply',
    status: 'unsupported',
    capabilities: { ...EMPTY_CANVAS_SURFACE_CAPABILITIES, schema: 3 },
  });
  expect(server.receive({ ...request, requestId: 3, schema: 0 })).toBe(false);
  server.dispose();
});

test('synchronous transport refusal stops its deadline, never retries and permits a later small request', async () => {
  const sent: CanvasMessage[] = [];
  const stop = jest.fn();
  const client = new CanvasClient(session, {
    send: (message) => {
      sent.push(message);
      if (message.type === 'surface.select')
        throw new Error('Native transport payload was refused.');
      if (message.type === 'surface.prepare')
        client.receive({ ...message, type: 'surface.reply', status: 'ready', message: '' });
    },
    later: () => stop,
    report: () => {},
  });
  await connectCanvasSurface(client, sent);
  stop.mockClear();
  await expect(
    negotiatedSurface(client).execute({
      type: 'surface.select',
      address,
      revision: 1,
      itemId: 'result:one',
      file: { filename: 'a.png', subfolder: '', type: 'output' },
    }),
  ).rejects.toThrow('payload was refused');
  expect(stop).toHaveBeenCalledTimes(1);
  expect(sent.filter(({ type }) => type === 'surface.select')).toHaveLength(1);
  expect(sent.filter(({ type }) => type === 'cancel')).toHaveLength(0);
  await expect(
    negotiatedSurface(client).execute({ type: 'surface.prepare', address, revision: 2 }),
  ).resolves.toMatchObject({ status: 'ready' });
  expect(stop).toHaveBeenCalledTimes(2);
  client.dispose();
});

test.each([
  { schema: 4 },
  { other: true },
  { imageFormats: ['unknown'] },
  { imageFormats: ['png', 'png', 'png', 'png'] },
  { viewportCommands: ['comparison'] },
  { previewFormats: ['webp'] },
  { viewportState: null },
  { input: 'yes' },
  { input: null },
])('rejects malformed capability descriptors %#', (patch) => {
  const request = {
    protocol: 'sugarcubes.canvas',
    version: 3,
    session,
    requestId: 1,
    type: 'surface.capabilities.request',
    schema: 2,
  } as const;
  expect(
    isCanvasMessage({
      ...capabilitiesReply(request),
      capabilities: { ...selectedPngCapabilities, ...patch },
    }),
  ).toBe(false);
});

/** Exercise the same discovery documents vendored by the Rust host. */
test('accepts shared valid discovery vectors and rejects each malformed variant', () => {
  const vectors = JSON.parse(
    readFileSync('integrations/contracts/canvas_surface_discovery_vectors.json', 'utf8'),
  ) as {
    request: unknown;
    futureRequest: unknown;
    available: unknown;
    unsupported: unknown;
    comparison: unknown;
    input: unknown;
    invalidRequests: unknown[];
    invalidReplies: unknown[];
  };
  for (const message of [
    vectors.request,
    vectors.futureRequest,
    vectors.available,
    vectors.unsupported,
    vectors.comparison,
    vectors.input,
  ])
    expect(isCanvasMessage(message)).toBe(true);
  for (const message of [...vectors.invalidRequests, ...vectors.invalidReplies])
    expect(isCanvasMessage(message)).toBe(false);
});

/** An unsupported older host has one bounded discovery attempt and cannot publish a late reply. */
test('times out discovery once and retains an explicit absent surface', async () => {
  const sent: CanvasMessage[] = [];
  const deadlines: (() => void)[] = [];
  const report = jest.fn();
  const stop = jest.fn();
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: (_duration, callback) => {
      deadlines.push(callback);
      return stop;
    },
    report,
  });
  const pending = client.readContext();
  client.receive({ ...sent[0], type: 'context.reply', context });
  await Promise.resolve();
  const request = sent[1];
  deadlines[1]?.();
  await pending;
  expect(client.surface).toBeNull();
  expect(sent.map(({ type }) => type)).toEqual([
    'context.request',
    'surface.capabilities.request',
    'cancel',
  ]);
  expect(stop).toHaveBeenCalledTimes(2);
  expect(report).toHaveBeenCalledTimes(1);
  expect(client.receive(capabilitiesReply(request))).toBe(false);
  client.dispose();
});

/** Page teardown retires discovery through the same client cancellation and deadline owner. */
test('disposal during discovery cancels it and rejects the owning context read', async () => {
  const f = fixture();
  const pending = f.client.readContext();
  contextReply(f, 0);
  await Promise.resolve();
  f.client.dispose();
  await expect(pending).rejects.toThrow('closed');
  expect(f.client.surface).toBeNull();
  expect(f.sent.map(({ type }) => type)).toEqual([
    'context.request',
    'surface.capabilities.request',
    'cancel',
  ]);
  expect(f.report).not.toHaveBeenCalled();
});

/** Admission snapshots declarations before resuming asynchronous request consumers. */
test('retains the admitted descriptor when transport-owned reply values mutate immediately', async () => {
  const f = fixture();
  const pending = f.client.readContext();
  const mutableContext = {
    ...context,
    canvas: { ...context.canvas, capabilities: { ...context.canvas.capabilities } },
  };
  contextReply(f, 0, mutableContext);
  mutableContext.canvas.capabilities.displayOutput = false;
  await Promise.resolve();
  const formats: ('png' | 'jpeg' | 'webp')[] = ['png'];
  const reply = {
    ...capabilitiesReply(f.sent[1]),
    capabilities: { ...selectedPngCapabilities, imageFormats: formats },
  };
  f.client.receive(reply);
  formats[0] = 'jpeg';
  await pending;
  expect(f.client.readCanvas()?.capabilities.displayOutput).toBe(true);
  expect(negotiatedSurface(f.client).capabilities.imageFormats).toEqual(['png']);
  f.client.dispose();
});
