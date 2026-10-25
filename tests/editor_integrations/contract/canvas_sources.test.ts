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
/** Prove reusable source capture, interpretation-neutral pixels and isolated capability negotiation. */
import { CanvasSourceConnection } from '../../../integrations/shared/CanvasSourceConnection.js';
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import {
  encodeCanvasRaster,
  decodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';
import type { CanvasSourceHost } from '../../../integrations/contracts/CanvasSources.js';
import type {
  CanvasMessage,
  CanvasSourceCommand,
  CanvasSourceResult,
  CanvasSourceChange,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

const session = '00000000-0000-4000-8000-000000000001';
const document = { id: 'document', geometryRevision: 'geometry', width: 2, height: 1 };
const pixels = encodeCanvasRaster(
  2,
  1,
  'rgba8',
  Uint8Array.from([10, 20, 30, 40, 200, 150, 100, 255]),
);

test('round-trips raw channels and selection coverage without a mask consumer', () => {
  expect([...decodeCanvasRaster(pixels)]).toEqual([10, 20, 30, 40, 200, 150, 100, 255]);
  expect([
    ...decodeCanvasRaster(encodeCanvasRaster(2, 1, 'coverage8', Uint8Array.from([0, 127]))),
  ]).toEqual([0, 127]);
  expect(() => encodeCanvasRaster(2, 1, 'rgba8', new Uint8Array(2))).toThrow('byte count');
  expect(() => encodeCanvasRaster(65536, 65536, 'rgba8', new Uint8Array())).toThrow('budget');
  expect(() => decodeCanvasRaster({ ...pixels, data: '<bad>' })).toThrow();
});

test('validates explicit layer targeting and keeps file access outside native source requests', () => {
  const request = {
    protocol: 'sugarcubes.canvas',
    version: 2,
    session,
    requestId: 1,
    type: 'source.request',
    command: {
      type: 'capture',
      document,
      targets: [{ kind: 'layer', layerId: 'layer-2' }],
      maximumSize: 288,
    },
  };
  expect(isCanvasMessage(request)).toBe(true);
  expect(
    isCanvasMessage({ ...request, command: { ...request.command, targets: [{ kind: 'layer' }] } }),
  ).toBe(false);
  expect(
    isCanvasMessage({
      ...request,
      command: { ...request.command, targets: [{ kind: 'file', file: 'private.png' }] },
    }),
  ).toBe(false);
  expect(
    isCanvasMessage({
      protocol: 'sugarcubes.canvas',
      version: 2,
      session,
      requestId: 0,
      type: 'source.changed',
      change: { documentId: document.id, revision: '2' },
    }),
  ).toBe(true);
});

/** Wire the real shared transport owners around a capable or deliberately limited native boundary. */
function fixture(capable: boolean) {
  const changes = new Set<(change: CanvasSourceChange) => void>(),
    sent: CanvasMessage[] = [];
  const execute = jest.fn(async (command: CanvasSourceCommand): Promise<CanvasSourceResult> => {
    if (command.type === 'context')
      return {
        kind: 'context',
        supported: true,
        document,
        revision: '1',
        targets: ['selection', 'current-layer', 'layer', 'composition', 'composition-from-layer'],
        layers: [
          { id: 'layer-2', name: 'Specific layer', parentId: null, depth: 0, visible: true },
        ],
        currentLayerId: 'layer-2',
      };
    if (command.type === 'capture')
      return {
        kind: 'captured',
        snapshots: command.targets.map((target) => ({ target, document, revision: '1', pixels })),
      };
    return { kind: 'watching' };
  });
  const sources: CanvasSourceHost = {
    canvasId: 'editor',
    execute,
    subscribe: (listener) => {
      changes.add(listener);
      return () => {
        changes.delete(listener);
      };
    },
  };
  const report = jest.fn();
  const server = new CanvasServer(session, {
    context: () => ({
      canvas: {
        id: 'editor',
        label: 'Editor',
        capabilities: { displayOutput: false, insertImageLayer: false, positionImageLayer: false },
      },
      document,
    }),
    insert: async () => ({ status: 'unsupported', message: 'Image insertion is unavailable.' }),
    ...(capable ? { sources } : {}),
    send: (message) => client.receive(message),
    report,
  });
  const client = new CanvasClient(session, {
    send: (message) => {
      sent.push(message);
      server.receive(message);
    },
    later: () => () => {},
    report,
  });
  return { client, server, execute, changes, sent, report };
}

test('captures source targets and shares one request sequence and native observation lifetime', async () => {
  const f = fixture(true);
  await f.client.readContext();
  const context = await f.client.sources.execute({ type: 'context' });
  expect(context.kind).toBe('context');
  expect(f.client.sources.canvasId).toBe('editor');
  const target = { kind: 'composition-from-layer', layerId: 'layer-2' } as const;
  const captured = await f.client.sources.execute({
    type: 'capture',
    document,
    targets: [target],
    maximumSize: 0,
  });
  expect(captured).toEqual({
    kind: 'captured',
    snapshots: [{ target, document, revision: '1', pixels }],
  });
  expect(
    f.sent.filter((message) => message.type !== 'cancel').map((message) => message.requestId),
  ).toEqual([1, 2, 3]);
  const first = jest.fn(),
    second = jest.fn();
  const stop = f.client.sources.subscribe(first),
    stop2 = f.client.sources.subscribe(second);
  for (let n = 0; n < 8; n++) await Promise.resolve();
  expect(f.changes.size).toBe(1);
  const change = { documentId: document.id, revision: '2' };
  for (const notify of f.changes) notify(change);
  expect(first).toHaveBeenLastCalledWith(change);
  expect(second).toHaveBeenLastCalledWith(change);
  stop();
  expect(f.changes.size).toBe(1);
  stop2();
  for (let n = 0; n < 8; n++) await Promise.resolve();
  expect(f.changes.size).toBe(0);
  f.client.dispose();
  f.server.dispose();
  expect(f.report).not.toHaveBeenCalled();
});

test('reports an unsupported source capability without inventing it for a limited canvas', async () => {
  const f = fixture(false);
  const context = await f.client.sources.execute({ type: 'context' });
  expect(context).toMatchObject({ kind: 'context', supported: false, targets: [], layers: [] });
  expect(
    await f.client.sources.execute({
      type: 'capture',
      document,
      targets: [{ kind: 'selection' }],
      maximumSize: 0,
    }),
  ).toMatchObject({ kind: 'failure', status: 'unsupported' });
  expect(f.execute).not.toHaveBeenCalled();
  f.client.dispose();
  f.server.dispose();
});

test('coalesces concurrent source context reads without caching later revisions', async () => {
  const pending: ((result: CanvasSourceResult) => void)[] = [];
  const execute = jest.fn(
    (_command: CanvasSourceCommand) =>
      new Promise<CanvasSourceResult>((resolve) => pending.push(resolve)),
  );
  const sources = new CanvasSourceConnection({
    identity: () => 'editor',
    execute,
    listen: () => () => {},
    report: jest.fn(),
  });
  const first = sources.execute({ type: 'context' }),
    second = sources.execute({ type: 'context' });
  expect(first).toBe(second);
  expect(execute).toHaveBeenCalledTimes(1);
  pending[0]!({ kind: 'failure', status: 'unavailable', message: 'No active document.' });
  await Promise.all([first, second]);
  const next = sources.execute({ type: 'context' });
  expect(execute).toHaveBeenCalledTimes(2);
  pending[1]!({ kind: 'failure', status: 'unavailable', message: 'No active document.' });
  await next;
  sources.dispose();
});
