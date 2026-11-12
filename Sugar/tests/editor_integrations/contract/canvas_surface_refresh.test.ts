/** Prove query-only refreshes preserve active grants and explicit invalidation fences every candidate. */
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import type { CanvasSurfaceCapabilities } from '../../../integrations/contracts/CanvasSurface.js';
import type {
  CanvasContext,
  CanvasMessage,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { EMPTY_CANVAS_SURFACE_CAPABILITIES } from '../../../integrations/shared/CanvasSurfaceDiscovery.js';
import {
  connectCanvasSurface,
  negotiatedSurface,
  selectedPngCapabilities,
} from '../support/canvas_surface.js';

const context: CanvasContext = {
  canvas: {
    id: 'native',
    label: 'Native',
    capabilities: { displayOutput: true, insertImageLayer: false, positionImageLayer: false },
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

/** Project reply correlation without copying request-only schema fields. */
function envelope(message: CanvasMessage) {
  if (!('requestId' in message)) throw new Error('Expected a correlated request.');
  const { protocol, version, session, requestId } = message;
  return { protocol, version, session, requestId };
}

/** Hold only host replies; all request identities and grant behavior use the real client. */
async function fixture(capabilities: CanvasSurfaceCapabilities = selectedPngCapabilities) {
  const sent: CanvasMessage[] = [];
  const report = jest.fn();
  const client = new CanvasClient('canvas-refresh-session', {
    send: (message) => sent.push(message),
    later: () => () => {},
    report,
  });
  await connectCanvasSurface(client, sent, capabilities);
  const surface = negotiatedSurface(client);
  return {
    client,
    sent,
    report,
    surface,
    context(value = context) {
      const request = sent.at(-1);
      if (request?.type !== 'context.request') throw new Error('Expected a context request.');
      client.receive({ ...request, type: 'context.reply', context: value });
    },
    capabilities(available = true, capabilities = selectedPngCapabilities) {
      const request = sent.at(-1);
      if (request?.type !== 'surface.capabilities.request') throw new Error('Expected discovery.');
      client.receive({
        protocol: request.protocol,
        version: request.version,
        session: request.session,
        requestId: request.requestId,
        type: 'surface.capabilities.reply',
        status: available ? 'available' : 'unavailable',
        capabilities: available ? capabilities : EMPTY_CANVAS_SURFACE_CAPABILITIES,
        message: '',
      });
    },
  };
}

test('an unchanged refresh retains the exact executable proxy before, during and after discovery', async () => {
  const f = await fixture();
  const refresh = f.client.refreshContext();
  expect(f.client.surface).toBe(f.surface);
  f.context();
  await Promise.resolve();
  expect(f.client.surface).toBe(f.surface);
  const prepare = f.surface.execute({
    type: 'surface.select',
    address,
    revision: 1,
    itemId: 'result:one',
    file: { filename: 'source.png', subfolder: '', type: 'output' },
  });
  const request = f.sent.at(-1);
  if (request?.type !== 'surface.select') throw new Error('Active grant was not executable.');
  expect(
    f.client.receive({
      ...envelope(request),
      address: request.address,
      revision: request.revision,
      type: 'surface.reply',
      status: 'applied',
      message: '',
    }),
  ).toBe(true);
  await expect(prepare).resolves.toMatchObject({ status: 'applied' });
  const discovery = f.sent
    .filter((message) => message.type === 'surface.capabilities.request')
    .at(-1)!;
  f.client.receive({
    ...envelope(discovery),
    type: 'surface.capabilities.reply',
    status: 'available',
    capabilities: selectedPngCapabilities,
    message: '',
  });
  await refresh;
  expect(f.client.surface).toBe(f.surface);
  expect(f.report).not.toHaveBeenCalled();
  f.client.dispose();
});

test('changed canvas identity revokes before waiting for its capabilities', async () => {
  const f = await fixture();
  const refresh = f.client.refreshContext();
  f.context({ ...context, canvas: { ...context.canvas, id: 'replacement' } });
  await Promise.resolve();
  expect(f.client.surface).toBeNull();
  await expect(
    f.surface.execute({ type: 'surface.prepare', address, revision: 1 }),
  ).resolves.toMatchObject({ status: 'unavailable' });
  f.capabilities();
  await refresh;
  expect(f.client.surface).not.toBe(f.surface);
  f.client.dispose();
});

test.each(['refusal', 'capabilities'] as const)(
  'a %s refresh replaces the active grant only at validated commit',
  async (change) => {
    const f = await fixture();
    const refresh = f.client.refreshContext();
    f.context();
    await Promise.resolve();
    expect(f.client.surface).toBe(f.surface);
    f.capabilities(change !== 'refusal', {
      ...selectedPngCapabilities,
      imageFormats: ['png', 'jpeg'],
    });
    await refresh;
    expect(f.client.surface).not.toBe(f.surface);
    if (change === 'refusal') expect(f.client.surface).toBeNull();
    await expect(
      f.surface.execute({ type: 'surface.prepare', address, revision: 1 }),
    ).resolves.toMatchObject({ status: 'unavailable' });
    f.client.dispose();
  },
);

test.each(['withdraw', 'read', 'dispose'] as const)(
  '%s supersedes a delayed refresh so its reply cannot resurrect a grant',
  async (action) => {
    const f = await fixture();
    const refresh = f.client.refreshContext();
    void refresh.catch(() => {});
    f.context();
    await Promise.resolve();
    const stale = f.sent.at(-1)!;
    let read: Promise<CanvasContext> | null = null;
    if (action === 'withdraw') f.client.withdrawSurface();
    if (action === 'read') read = f.client.readContext();
    if (action === 'dispose') f.client.dispose();
    expect(f.client.surface).toBeNull();
    f.client.receive({
      ...envelope(stale),
      type: 'surface.capabilities.reply',
      status: 'available',
      capabilities: selectedPngCapabilities,
      message: '',
    });
    if (action === 'dispose') await expect(refresh).rejects.toThrow('closed');
    else await refresh;
    expect(f.client.surface).toBeNull();
    if (read) {
      f.context({
        ...context,
        canvas: {
          ...context.canvas,
          capabilities: { ...context.canvas.capabilities, displayOutput: false },
        },
      });
      await read;
    }
    f.client.dispose();
  },
);

test('a failed current query revokes its grant and preserves the failure cause', async () => {
  const f = await fixture();
  const refresh = f.client.refreshContext();
  const request = f.sent.at(-1)!;
  f.client.receive({
    ...request,
    type: 'error.reply',
    failure: { status: 'unavailable', message: 'Backend grant changed.' },
  });
  await expect(refresh).rejects.toThrow('Backend grant changed.');
  expect(f.client.surface).toBeNull();
  await expect(
    f.surface.execute({ type: 'surface.prepare', address, revision: 1 }),
  ).resolves.toMatchObject({ status: 'unavailable' });
  f.client.dispose();
});

test('a newer refresh owns publication while an older refused capability reply cannot withdraw it', async () => {
  const f = await fixture();
  const older = f.client.refreshContext();
  f.context();
  await Promise.resolve();
  const stale = f.sent.at(-1)!;
  const newer = f.client.refreshContext();
  f.context();
  await Promise.resolve();
  f.capabilities(true, { ...selectedPngCapabilities, imageFormats: ['png', 'jpeg'] });
  await newer;
  const replacement = f.client.surface;
  expect(replacement).not.toBe(f.surface);
  f.client.receive({
    ...envelope(stale),
    type: 'surface.capabilities.reply',
    status: 'unavailable',
    capabilities: EMPTY_CANVAS_SURFACE_CAPABILITIES,
    message: '',
  });
  await older;
  expect(f.client.surface).toBe(replacement);
  f.client.dispose();
});

test('an obsolete context failure cannot withdraw a grant kept by the current refresh', async () => {
  const f = await fixture();
  const older = f.client.refreshContext();
  const stale = f.sent.at(-1)!;
  const newer = f.client.refreshContext();
  f.context();
  await Promise.resolve();
  f.capabilities();
  await newer;
  f.client.receive({
    ...envelope(stale),
    type: 'error.reply',
    failure: { status: 'unavailable', message: 'Obsolete failure.' },
  });
  await expect(older).rejects.toThrow('Obsolete failure.');
  expect(f.client.surface).toBe(f.surface);
  f.client.dispose();
});

test('changed context support withdraws immediately without optional capability discovery', async () => {
  const f = await fixture();
  const refresh = f.client.refreshContext();
  f.context({
    ...context,
    canvas: {
      ...context.canvas,
      capabilities: { ...context.canvas.capabilities, displayOutput: false },
    },
  });
  await refresh;
  expect(f.client.surface).toBeNull();
  expect(f.sent.at(-1)?.type).toBe('context.request');
  f.client.dispose();
});

test('metadata and JSON property order changes preserve the same native grant', async () => {
  const f = await fixture();
  const refresh = f.client.refreshContext();
  f.context({
    canvas: {
      label: context.canvas.label,
      capabilities: { positionImageLayer: false, insertImageLayer: false, displayOutput: true },
      id: context.canvas.id,
    },
    document: { id: 'input-document', geometryRevision: '2', width: 100, height: 100 },
  });
  await Promise.resolve();
  expect(f.client.surface).toBe(f.surface);
  const capabilities = selectedPngCapabilities;
  f.capabilities(true, {
    input: capabilities.input,
    overview: capabilities.overview,
    comparison: capabilities.comparison,
    previewFormats: capabilities.previewFormats,
    viewportState: capabilities.viewportState,
    viewportCommands: capabilities.viewportCommands,
    imageFormats: capabilities.imageFormats,
    schema: capabilities.schema,
  });
  await refresh;
  expect(f.client.surface).toBe(f.surface);
  expect(f.client.readCanvas()?.id).toBe(context.canvas.id);
  f.client.dispose();
});

test('an identical refresh retains the existing accepted presentation observation', async () => {
  const capabilities: CanvasSurfaceCapabilities = {
    ...selectedPngCapabilities,
    previewFormats: ['jpeg'],
  };
  const f = await fixture(capabilities);
  const observation = f.surface.presentation;
  if (!observation) throw new Error('Presentation was not negotiated.');
  const subscription = observation.subscribe(() => {});
  const watch = f.sent.at(-1)!;
  f.client.receive({
    ...envelope(watch),
    type: 'surface.presentation.reply',
    schema: 1,
    status: 'available',
    enabled: true,
    message: '',
  });
  await expect(subscription.ready).resolves.toBe('observing');
  const refresh = f.client.refreshContext();
  f.context();
  await Promise.resolve();
  f.capabilities(true, capabilities);
  await refresh;
  expect(f.client.surface).toBe(f.surface);
  expect(f.client.surface?.presentation).toBe(observation);
  expect(f.sent.filter((message) => message.type === 'surface.presentation.request')).toHaveLength(
    1,
  );
  subscription.stop();
  const stop = f.sent.at(-1)!;
  f.client.receive({
    ...envelope(stop),
    type: 'surface.presentation.reply',
    schema: 1,
    status: 'available',
    enabled: false,
    message: '',
  });
  await Promise.resolve();
  f.client.dispose();
});
