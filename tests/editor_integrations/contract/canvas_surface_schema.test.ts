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
/** Refuse incompatible surface peers before binding a camera or invoking mutations. */
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { negotiatesCanvasSurfaceInspection } from '../../../integrations/shared/CanvasSurfaceViewport.js';
import { isCanvasSurfaceInspection } from '../../../integrations/shared/CanvasSurfaceValidation.generated.js';
import { selectedPngCapabilities } from '../support/canvas_surface.js';

const session = 'surface-schema-session';
const context = {
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

test.each([
  { requestSchema: 1, hostSchema: 2 },
  { requestSchema: 2, hostSchema: 1 },
])(
  'refuses request schema $requestSchema against host schema $hostSchema before native work',
  async ({ requestSchema, hostSchema }) => {
    const replies: CanvasMessage[] = [];
    const execute = jest.fn<() => Promise<{ status: 'applied'; message: string }>>();
    const read = jest.fn(async () => context);
    const server = new CanvasServer(session, {
      context: read,
      insert: async () => {
        throw new Error('Unexpected insertion.');
      },
      surface: { capabilities: { ...selectedPngCapabilities, schema: hostSchema }, execute },
      send: (message) => replies.push(message),
      report: (cause) => {
        throw cause;
      },
    });
    try {
      expect(
        server.receive({
          protocol: 'sugarcubes.canvas',
          version: 3,
          session,
          requestId: 1,
          type: 'surface.capabilities.request',
          schema: requestSchema,
        }),
      ).toBe(true);
      await Promise.resolve();
      expect(replies).toEqual([
        expect.objectContaining({
          type: 'surface.capabilities.reply',
          status: 'unsupported',
          capabilities: { ...selectedPngCapabilities, schema: requestSchema, imageFormats: [] },
          message: expect.stringContaining('Update SugarCubes and its native host together'),
        }),
      ]);
      expect(read).not.toHaveBeenCalled();
      expect(execute).not.toHaveBeenCalled();
      expect(
        server.receive({
          protocol: 'sugarcubes.canvas',
          version: 3,
          session,
          requestId: 2,
          type: 'surface.select',
          revision: 1,
          itemId: 'result:one',
          address: {
            sessionId: 'canvas:workflow',
            viewId: 'view:one',
            windowId: 'window:one',
            attachmentId: 'attachment:one',
            generation: 1,
          },
          file: { filename: 'one.png', subfolder: '', type: 'output' },
        }),
      ).toBe(true);
      expect(replies[1]).toMatchObject({
        type: 'surface.reply',
        status: 'unavailable',
        message: expect.stringContaining('Native surface schema 2 is required'),
      });
      expect(execute).not.toHaveBeenCalled();
    } finally {
      server.dispose();
    }
  },
);

test('rejects a legacy available descriptor before exposing any surface mutation', async () => {
  const sent: CanvasMessage[] = [];
  const report = jest.fn<(cause: unknown) => void>();
  const client = new CanvasClient(session, {
    send: (message) => {
      sent.push(message);
      if (message.type === 'context.request')
        client.receive({ ...message, type: 'context.reply', context });
      else if (message.type === 'surface.capabilities.request') {
        expect(message.schema).toBe(2);
        client.receive({
          protocol: message.protocol,
          version: message.version,
          session,
          requestId: message.requestId,
          type: 'surface.capabilities.reply',
          status: 'available',
          capabilities: { ...selectedPngCapabilities, schema: 1 },
          message: '',
        });
      }
    },
    later: () => () => {},
    report,
  });
  try {
    await client.readContext();
    expect(client.surface).toBeNull();
    expect(report).toHaveBeenCalledWith(
      expect.objectContaining({
        message: expect.stringContaining('Update SugarCubes and its native host together'),
      }),
    );
    expect(sent.map(({ type }) => type)).toEqual([
      'context.request',
      'surface.capabilities.request',
    ]);
  } finally {
    client.dispose();
  }
});

test('reports an unsupported schema-two host at the injected discovery deadline', async () => {
  let expire = (): void => {
    throw new Error('No discovery deadline.');
  };
  const report = jest.fn<(cause: unknown) => void>();
  const client = new CanvasClient(session, {
    send: (message) => {
      if (message.type === 'context.request')
        client.receive({ ...message, type: 'context.reply', context });
    },
    later: (_milliseconds, callback) => {
      expire = callback;
      return () => {};
    },
    report,
  });
  try {
    const connected = client.readContext();
    await Promise.resolve();
    expire();
    await connected;
    expect(client.surface).toBeNull();
    expect(report).toHaveBeenCalledWith(
      expect.objectContaining({
        message: expect.stringContaining('Native surface schema 2 is unsupported or unavailable'),
      }),
    );
  } finally {
    client.dispose();
  }
});

test('legacy descriptors never negotiate inspections even when they name the command', () => {
  expect(
    negotiatesCanvasSurfaceInspection({
      ...selectedPngCapabilities,
      schema: 1,
      viewportState: true,
      viewportCommands: ['inspect'],
    }),
  ).toBe(false);
});

test.each([Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY])(
  'rejects nonfinite camera centers without normalizing them: %s',
  (value) => {
    expect(
      isCanvasSurfaceInspection({ center: { x: value, y: 0.5 }, span: { x: 0.001, y: 0.001 } }),
    ).toBe(false);
    expect(
      isCanvasSurfaceInspection({ center: { x: 0.5, y: value }, span: { x: 0.001, y: 0.001 } }),
    ).toBe(false);
  },
);
