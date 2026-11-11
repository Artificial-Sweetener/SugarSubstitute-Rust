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
/**
 * Prove discovery steps down between releases instead of switching the surface off.
 *
 * One conformance suite runs a capable host and deliberately limited in-memory hosts through the
 * real page client and host server, in both directions: a page newer than its host, and a host
 * asked for a schema newer than it knows.
 */
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { negotiatesCanvasSurfaceBackdrop } from '../../../integrations/shared/CanvasSurfaceBackdrop.js';
import { SURFACE_SCHEMA_REQUIRED } from '../../../integrations/shared/CanvasSurfaceDiscovery.js';
import { NEWEST_CANVAS_SURFACE_SCHEMA } from '../../../integrations/shared/CanvasSurfaceSchemas.js';
import type {
  CanvasContext,
  CanvasMessage,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfaceResult,
} from '../../../integrations/contracts/CanvasSurface.js';
import { connectCanvasSurface, selectedPngCapabilities } from '../support/canvas_surface.js';

const session = 'surface-schema-step-down';
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
const backdropGeometry: CanvasSurfaceCommand = {
  type: 'surface.geometry',
  revision: 1,
  address,
  geometry: {
    coordinateSpace: 'client',
    units: 'css-pixels',
    origin: { x: 0, y: 0 },
    viewport: { width: 800, height: 600 },
    clip: { x: 0, y: 0, width: 800, height: 600 },
    physicalPixelsPerCssPixel: 1,
    visible: true,
    focused: false,
    backdrops: [{ x: 8, y: 552, width: 240, height: 40, cornerRadius: 8, sigma: 10 }],
  },
};
const declared: CanvasSurfaceCapabilities = {
  ...selectedPngCapabilities,
  schema: 3,
  viewportCommands: ['fit', 'zoom'],
  backdrop: { schema: 1 },
};

/** Each host declares what it implements; older hosts declare an older schema. */
const hosts = [
  { name: 'a capable host', schema: 3, expressed: 3, backdrop: true },
  { name: 'a schema-two host', schema: 2, expressed: 2, backdrop: false },
  { name: 'a host older than the native surface', schema: 1, expressed: null, backdrop: false },
] as const;
type Host = (typeof hosts)[number];

/** Connect the real page client to the real host server through a synchronous in-memory transport. */
function application(host: Host) {
  const executed: CanvasSurfaceCommand[] = [];
  const toClient: CanvasMessage[] = [];
  const pageReport = jest.fn<(error: unknown) => void>();
  const hostReport = jest.fn<(error: unknown) => void>();
  const server = new CanvasServer(session, {
    context: async () => context,
    insert: async () => {
      throw new Error('Layer insertion is not part of discovery.');
    },
    surface: {
      capabilities: { ...declared, schema: host.schema },
      execute: async (command): Promise<CanvasSurfaceResult> => {
        executed.push(command);
        return { status: command.type === 'surface.prepare' ? 'ready' : 'applied', message: '' };
      },
    },
    send: (message) => toClient.push(message),
    report: hostReport,
  });
  const client = new CanvasClient(session, {
    send: (message) => {
      server.receive(message);
    },
    later: () => () => {},
    report: pageReport,
  });
  /** Deliver host replies as the asynchronous host work settles. */
  const settle = async (): Promise<void> => {
    for (let turn = 0; turn < 20; turn += 1) {
      await Promise.resolve();
      while (toClient.length) client.receive(toClient.shift());
    }
  };
  return { server, client, executed, toClient, pageReport, hostReport, settle };
}

describe.each(hosts)('discovery against $name', (host) => {
  test('a page requesting the newest schema steps down to what the host expresses', async () => {
    const app = application(host);
    const pending = app.client.readContext();
    await app.settle();
    await pending;
    const surface = app.client.surface;
    if (host.expressed === null) expect(surface).toBeNull();
    else {
      expect(surface?.capabilities.schema).toBe(host.expressed);
      expect(surface?.capabilities.imageFormats).toEqual(['png']);
      expect(surface?.capabilities.viewportCommands).toEqual(['fit', 'zoom']);
      expect(surface && negotiatesCanvasSurfaceBackdrop(surface.capabilities)).toBe(host.backdrop);
    }
    expect(app.pageReport).not.toHaveBeenCalled();
    expect(app.hostReport).not.toHaveBeenCalled();
    app.client.dispose();
    app.server.dispose();
  });

  test('a feature the answered schema cannot express is switched off before reaching the host', async () => {
    const app = application(host);
    const pending = app.client.readContext();
    await app.settle();
    await pending;
    const surface = app.client.surface;
    if (!surface) {
      expect(host.expressed).toBeNull();
      return;
    }
    const result = surface.execute(backdropGeometry);
    await app.settle();
    if (host.backdrop) {
      await expect(result).resolves.toMatchObject({ status: 'applied' });
      expect(app.executed.map(({ type }) => type)).toEqual(['surface.geometry']);
    } else {
      await expect(result).resolves.toMatchObject({ status: 'unavailable' });
      expect(app.executed).toEqual([]);
    }
    app.client.dispose();
    app.server.dispose();
  });

  test.each([NEWEST_CANVAS_SURFACE_SCHEMA + 1, 2147483647])(
    'a host asked for unknown schema %i answers with the newest it knows',
    async (requested) => {
      const app = application(host);
      expect(
        app.server.receive({
          protocol: 'sugarcubes.canvas',
          version: 3,
          session,
          requestId: 1,
          type: 'surface.capabilities.request',
          schema: requested,
        }),
      ).toBe(true);
      for (let turn = 0; turn < 20; turn += 1) await Promise.resolve();
      const reply = app.toClient.shift();
      if (reply?.type !== 'surface.capabilities.reply') throw new Error('Missing discovery reply.');
      expect(reply.capabilities.schema).toBe(host.expressed ?? host.schema);
      expect(reply.status).toBe(host.expressed === null ? 'unsupported' : 'available');
      expect(reply.capabilities.backdrop !== undefined).toBe(host.backdrop);
      expect(app.hostReport).not.toHaveBeenCalled();
      app.server.dispose();
    },
  );
});

test('a newer page keeps using the surface after a host answers its unknown schema', async () => {
  const app = application(hosts[0]);
  app.server.receive({
    protocol: 'sugarcubes.canvas',
    version: 3,
    session,
    requestId: 1,
    type: 'surface.capabilities.request',
    schema: NEWEST_CANVAS_SURFACE_SCHEMA + 1,
  });
  for (let turn = 0; turn < 20; turn += 1) await Promise.resolve();
  app.server.receive({
    protocol: 'sugarcubes.canvas',
    version: 3,
    session,
    requestId: 2,
    type: 'surface.prepare',
    address,
    revision: 1,
  });
  for (let turn = 0; turn < 20; turn += 1) await Promise.resolve();
  expect(app.executed.map(({ type }) => type)).toEqual(['surface.prepare']);
  expect(app.toClient.at(-1)).toMatchObject({ type: 'surface.reply', status: 'ready' });
  app.server.dispose();
});

/** A host outside this repository may send declarations its answered schema cannot carry. */
test('the page drops declarations an older answered schema cannot express', async () => {
  const sent: CanvasMessage[] = [];
  const report = jest.fn<(error: unknown) => void>();
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => () => {},
    report,
  });
  await connectCanvasSurface(client, sent, { ...declared, schema: 2 });
  expect(client.surface?.capabilities.schema).toBe(2);
  expect(client.surface?.capabilities.backdrop).toBeUndefined();
  await expect(client.surface?.execute(backdropGeometry)).resolves.toMatchObject({
    status: 'unavailable',
  });
  expect(sent.filter(({ type }) => type === 'surface.geometry')).toEqual([]);
  expect(report).not.toHaveBeenCalled();
  client.dispose();
});

/** A real pre-surface host answers unsupported, so an available answer is diagnosed but harmless. */
test('an available answer in a schema that expresses no surface leaves it absent', async () => {
  const sent: CanvasMessage[] = [];
  const report = jest.fn<(error: unknown) => void>();
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => () => {},
    report,
  });
  await connectCanvasSurface(client, sent, { ...selectedPngCapabilities, schema: 1 });
  expect(client.surface).toBeNull();
  expect(report).toHaveBeenCalledWith(
    expect.objectContaining({ message: SURFACE_SCHEMA_REQUIRED }),
  );
  client.dispose();
});

test('the page requests the newest schema it knows', async () => {
  const sent: CanvasMessage[] = [];
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: () => {},
  });
  await connectCanvasSurface(client, sent);
  expect(sent.find(({ type }) => type === 'surface.capabilities.request')).toMatchObject({
    schema: NEWEST_CANVAS_SURFACE_SCHEMA,
  });
  client.dispose();
});

/**
 * A host newer than this page knows schema four and answers min(requested, newest it knows).
 * The fixture implements only that published rule, so the page must negotiate its own newest
 * schema with every schema-three feature available.
 */
test('a newer host answers this page in the schema the page requested', async () => {
  const HOST_NEWEST_SCHEMA = NEWEST_CANVAS_SURFACE_SCHEMA + 1;
  const sent: CanvasMessage[] = [];
  const report = jest.fn<(error: unknown) => void>();
  const client: CanvasClient = new CanvasClient(session, {
    send: (message) => {
      sent.push(message);
      if (message.type === 'context.request')
        queueMicrotask(() => client.receive({ ...message, type: 'context.reply', context }));
      if (message.type === 'surface.capabilities.request')
        queueMicrotask(() =>
          client.receive({
            protocol: message.protocol,
            version: message.version,
            session: message.session,
            requestId: message.requestId,
            type: 'surface.capabilities.reply',
            status: 'available',
            capabilities: {
              ...declared,
              schema: Math.min(message.schema, HOST_NEWEST_SCHEMA),
            },
            message: '',
          }),
        );
    },
    later: () => () => {},
    report,
  });
  await client.readContext();
  const surface = client.surface;
  expect(surface?.capabilities.schema).toBe(NEWEST_CANVAS_SURFACE_SCHEMA);
  expect(surface && negotiatesCanvasSurfaceBackdrop(surface.capabilities)).toBe(true);
  expect(report).not.toHaveBeenCalled();
  client.dispose();
});
