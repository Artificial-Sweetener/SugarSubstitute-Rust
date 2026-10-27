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
/** Prove portable capability discovery, insertion and replay safety across peer canvases. */
import { readFileSync } from 'node:fs';
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { WEB_CANVAS } from '../../../integrations/contracts/CanvasHost.js';
import { centerCanvasImage } from '../../../integrations/shared/CanvasPlacement.js';
import { canvasPngDimensions } from '../../../integrations/shared/CanvasImagePolicy.js';
import {
  encodeCanvasImage,
  decodeCanvasImage,
} from '../../../integrations/shared/CanvasImageEncoding.js';
import { canvasBindings } from '../../../scripts/integrations/generate-canvas-contract.js';
import type {
  CanvasContext,
  CanvasInsertImage,
  CanvasMessage,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
const session = '00000000-0000-4000-8000-000000000001';
const document = { id: 'doc-1', geometryRevision: 'geometry-1', width: 512, height: 384 };
const image: CanvasInsertImage = {
  document,
  image: { format: 'png' as const, encoding: 'base64' as const, data: 'fixture-png' },
  layerName: 'Generated image',
  position: null,
};
const result = {
  status: 'inserted',
  documentId: 'doc-1',
  layerId: 'layer-1',
  left: -128,
  top: -64,
  width: 768,
  height: 512,
} as const;
/** Compose a native-independent host with asynchronous context and controllable mutation. */
function application(supported = true, positioned = false) {
  const sent: CanvasMessage[] = [],
    replies: CanvasMessage[] = [],
    errors: unknown[] = [];
  const context: CanvasContext = {
    canvas: supported
      ? {
          id: 'test-editor',
          label: 'Test editor',
          capabilities: {
            displayOutput: false,
            insertImageLayer: true,
            positionImageLayer: positioned,
          },
        }
      : WEB_CANVAS,
    document: supported ? document : null,
  };
  const insert = jest.fn(async (_image: CanvasInsertImage, _cancel: CanvasCancellation) => result);
  const host = new CanvasServer(session, {
    context: () => Promise.resolve(context),
    insert,
    send: (value) => {
      replies.push(value);
      client.receive(value);
    },
    report: (error) => errors.push(error),
  });
  const client = new CanvasClient(session, {
    send: (value) => {
      sent.push(value);
      host.receive(value);
    },
    later: (ms, callback) => {
      const timer = setTimeout(callback, ms);
      return () => clearTimeout(timer);
    },
    report: (error) => errors.push(error),
  });
  return { host, client, sent, replies, errors, insert, context };
}
test.each([true, false])(
  'discovers actual peer capabilities without adding layer support to the web canvas: %s',
  async (supported) => {
    const app = application(supported);
    expect(app.client.readCanvas()).toBeNull();
    expect(await app.client.readContext()).toEqual(app.context);
    expect(app.client.readCanvas()?.capabilities.insertImageLayer).toBe(supported);
    expect((await app.client.insertImage(image)).status).toBe(
      supported ? 'inserted' : 'unsupported',
    );
    expect(app.insert).toHaveBeenCalledTimes(supported ? 1 : 0);
    app.client.dispose();
    app.host.dispose();
  },
);
test('does not repeat a mutation for duplicate, reordered or evicted requests', async () => {
  const app = application();
  expect(await app.client.insertImage(image)).toEqual(result);
  const request = app.sent[0];
  expect(request?.type).toBe('image.insert');
  app.host.receive(request);
  app.host.receive({
    image: {
      layerName: image.layerName,
      image: image.image,
      document: { height: 384, width: 512, geometryRevision: 'geometry-1', id: 'doc-1' },
      position: null,
    },
    type: 'image.insert',
    requestId: 1,
    session,
    version: 3,
    protocol: 'sugarcubes.canvas',
  });
  expect(app.insert).toHaveBeenCalledTimes(1);
  app.host.receive({ ...request, image: { ...image, layerName: 'Different' } });
  expect(app.replies.at(-1)).toMatchObject({ type: 'error.reply', failure: { status: 'stale' } });
  for (let requestId = 2; requestId < 40; requestId++)
    app.host.receive({
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId,
      type: 'context.request',
    });
  app.host.receive(request);
  expect(app.insert).toHaveBeenCalledTimes(1);
  expect(app.replies.at(-1)).toMatchObject({ type: 'error.reply', failure: { status: 'stale' } });
  app.client.dispose();
  app.host.dispose();
});

test('negotiates positioned insertion independently and refuses replay with a different origin', async () => {
  const limited = application();
  const positioned = { ...image, position: { left: 27, top: 41 } };
  expect(await limited.client.insertImage(positioned)).toMatchObject({ status: 'unsupported' });
  expect(limited.insert).not.toHaveBeenCalled();
  limited.client.dispose();
  limited.host.dispose();
  const capable = application(true, true);
  expect(await capable.client.insertImage(positioned)).toEqual(result);
  expect(capable.insert.mock.calls[0]?.[0].position).toEqual(positioned.position);
  const request = capable.sent[0];
  capable.host.receive({ ...request, image: { ...positioned, position: { left: 28, top: 41 } } });
  expect(capable.replies.at(-1)).toMatchObject({
    type: 'error.reply',
    failure: { status: 'stale' },
  });
  expect(capable.insert).toHaveBeenCalledTimes(1);
  capable.client.dispose();
  capable.host.dispose();
});
test('rejects hostile envelopes before touching an editor', () => {
  const app = application();
  for (const value of [
    null,
    {},
    { protocol: 'sugarcubes.canvas', version: 3, session, requestId: 0, type: 'context.request' },
    {
      protocol: 'sugarcubes.canvas',
      version: 3,
      session: 'wrong-session-0000',
      requestId: 1,
      type: 'image.insert',
      image,
    },
    {
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId: 1,
      type: 'image.insert',
      image: { ...image, extra: true },
    },
  ])
    expect(app.host.receive(value)).toBe(false);
  expect(app.insert).not.toHaveBeenCalled();
  app.client.dispose();
  app.host.dispose();
});
test('cancels owned work on caller cancellation and refuses late replies after disposal', async () => {
  const app = application();
  let finish: (() => void) | undefined;
  app.insert.mockImplementation(async (_image, cancellation) => {
    await new Promise<void>((resolve) => {
      finish = resolve;
    });
    expect(cancellation.cancelled()).toBe(true);
    return result;
  });
  const cancellation = new CanvasCancellation();
  const pending = app.client.insertImage(image, cancellation);
  await Promise.resolve();
  expect(app.insert).toHaveBeenCalledTimes(1);
  cancellation.cancel();
  expect(await pending).toMatchObject({ status: 'cancelled' });
  app.host.dispose();
  app.client.dispose();
  finish?.();
  await Promise.resolve();
  await Promise.resolve();
  expect(app.replies).toEqual([]);
  await expect(app.client.readContext()).rejects.toThrow('closed');
});
test('bounds deadlines and reports cancellation transport errors without leaving requests pending', async () => {
  jest.useFakeTimers();
  try {
    const errors: unknown[] = [];
    const client = new CanvasClient(session, {
      send: (value) => {
        if (value.type === 'cancel') throw new Error('disconnected');
      },
      later: (ms, callback) => {
        const timer = setTimeout(callback, ms);
        return () => clearTimeout(timer);
      },
      report: (error) => errors.push(error),
    });
    const pending = expect(client.insertImage(image)).rejects.toThrow('timed out');
    jest.advanceTimersByTime(30000);
    await pending;
    expect(errors).toHaveLength(1);
    client.dispose();
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    jest.useRealTimers();
  }
});
test.each([
  [768, 512, -128, -64],
  [256, 128, 128, 128],
  [513, 385, -1, -1],
  [512, 384, 0, 0],
])('centers intrinsic pixels without scaling %i x %i', (width, height, left, top) => {
  expect(centerCanvasImage(document, { width, height })).toEqual({ left, top });
});
test('rejects malformed PNG headers and decoded pixel budgets before native decoding', () => {
  const bytes = new Uint8Array(33);
  bytes.set([137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82]);
  const view = new DataView(bytes.buffer);
  view.setUint32(16, 768);
  view.setUint32(20, 512);
  expect(canvasPngDimensions(bytes)).toEqual({ width: 768, height: 512 });
  view.setUint32(16, 30000);
  view.setUint32(20, 30000);
  expect(() => canvasPngDimensions(bytes)).toThrow('pixel budget');
  expect(() => canvasPngDimensions(new Uint8Array())).toThrow('PNG');
});
test('keeps generated portable declarations and validators synchronized with the authoritative schema', async () => {
  const outputs = await canvasBindings(
    readFileSync('integrations/contracts/canvas.schema.json', 'utf8'),
  );
  for (const [path, source] of Object.entries(outputs))
    expect(readFileSync(path, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test.each([33, 34, 35, 16385, 100000])(
  'transports exact PNG bytes through portable base64 without browser or editor globals: %i',
  (size) => {
    const bytes = new Uint8Array(size);
    bytes.set([137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82]);
    const view = new DataView(bytes.buffer);
    view.setUint32(16, 768);
    view.setUint32(20, 512);
    for (let index = 33; index < size; index++) bytes[index] = index % 256;
    const image = encodeCanvasImage(bytes);
    expect(image.data).toBe(Buffer.from(bytes).toString('base64'));
    expect(decodeCanvasImage(image)).toEqual(bytes);
  },
);
test.each(['', 'a', '====', '!!!!', 'AAAA====', 'AA==AAAA', 'AAB='])(
  'rejects malformed or noncanonical encoded data before a native decoder: %s',
  (data) => {
    expect(() => decodeCanvasImage({ format: 'png', encoding: 'base64', data })).toThrow();
  },
);
