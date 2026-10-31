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
/** Qualify native camera inspection through shared vectors and the real canvas session. */
import { readFileSync } from 'node:fs';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import {
  isCanvasSurfaceSelectRequest,
  isCanvasSurfaceViewportReply,
  isCanvasSurfaceViewportRequest,
} from '../../../integrations/shared/CanvasSurfaceValidation.generated.js';
import { negotiatesCanvasSurfaceInspection } from '../../../integrations/shared/CanvasSurfaceViewport.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfaceInspection,
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
  inspection: CanvasSurfaceInspection;
  validRequests: Vector[];
  invalidRequests: Vector[];
  validReplies: Vector[];
  invalidReplies: Vector[];
  capabilities: Record<string, unknown>;
} = JSON.parse(
  readFileSync(
    new URL('../support/canvas_surface_inspection_vectors.json', import.meta.url),
    'utf8',
  ),
);
const session = 'surface-inspection-session';
const address = {
  sessionId: 'canvas:workflow',
  viewId: 'view:one',
  windowId: 'window:one',
  attachmentId: 'attachment:one',
  generation: 1,
} as const;
const file = { filename: 'ComfyUI_00001_.png', subfolder: '', type: 'output' } as const;
const inspecting: CanvasSurfaceCapabilities = {
  ...selectedPngCapabilities,
  viewportCommands: ['fit', 'native', 'zoom', 'pan', 'inspect'],
  viewportState: true,
};
const viewport = {
  mode: 'custom',
  physicalScale: 2,
  dimensions: { width: 1000, height: 800 },
  center: { x: 0.25, y: 0.75 },
} as const;

/** Connect a client whose surface operations remain under the test's control. */
async function fixture(declared: CanvasSurfaceCapabilities) {
  const sent: CanvasMessage[] = [];
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: (error) => {
      throw error;
    },
  });
  await connectCanvasSurface(client, sent, declared);
  sent.length = 0;
  return { client, sent };
}

/** Restore one inspection through a selection or the shown image's inspect command. */
function commands(inspection: CanvasSurfaceInspection): readonly CanvasSurfaceCommand[] {
  return [
    { type: 'surface.select', address, revision: 3, itemId: 'result:1', file, inspection },
    {
      type: 'surface.viewport.request',
      address,
      revision: 3,
      command: { type: 'inspect', inspection },
    },
  ];
}

test.each(vectors.validRequests)('admits inspection vector: $name', ({ value }) => {
  expect(isCanvasMessage(value)).toBe(true);
  expect(isCanvasSurfaceSelectRequest(value) || isCanvasSurfaceViewportRequest(value)).toBe(true);
});

test.each(vectors.invalidRequests)('refuses malformed inspection vector: $name', ({ value }) => {
  expect(isCanvasMessage(value)).toBe(false);
});

test.each(vectors.validReplies)('admits viewport state vector: $name', ({ value }) => {
  expect(isCanvasMessage(value)).toBe(true);
  expect(isCanvasSurfaceViewportReply(value)).toBe(true);
});

test.each(vectors.invalidReplies)('refuses malformed viewport state vector: $name', ({ value }) => {
  expect(isCanvasMessage(value)).toBe(false);
});

test('declares inspection only with authoritative viewport state and the inspect command', () => {
  expect(isCanvasMessage(vectors.capabilities)).toBe(true);
  expect(negotiatesCanvasSurfaceInspection(inspecting)).toBe(true);
  expect(negotiatesCanvasSurfaceInspection({ ...inspecting, viewportState: false })).toBe(false);
  expect(
    negotiatesCanvasSurfaceInspection({ ...inspecting, viewportCommands: ['fit', 'native'] }),
  ).toBe(false);
});

test.each([
  selectedPngCapabilities,
  { ...inspecting, viewportState: false },
  { ...inspecting, viewportCommands: ['fit', 'native', 'zoom', 'pan'] as const },
])('refuses undeclared inspection locally without occupying transport %#', async (declared) => {
  const f = await fixture(declared);
  try {
    for (const command of commands(vectors.inspection))
      await expect(negotiatedSurface(f.client).execute(command)).resolves.toMatchObject({
        status: 'unavailable',
      });
    expect(f.sent).toEqual([]);
    const plain = negotiatedSurface(f.client).execute({
      type: 'surface.select',
      address,
      revision: 3,
      itemId: 'result:1',
      file,
    });
    expect(f.sent.map(({ type }) => type)).toEqual(['surface.select']);
    f.client.dispose();
    await expect(plain).rejects.toThrow('closed');
  } finally {
    f.client.dispose();
  }
});

test('forwards a declared inspection and retains the reported center as immutable state', async () => {
  const f = await fixture(inspecting);
  try {
    const [select, inspect] = commands(vectors.inspection);
    if (!select || !inspect) throw new Error('Missing inspection commands.');
    const selected = negotiatedSurface(f.client).execute(select);
    expect(f.sent.at(-1)).toMatchObject({ type: 'surface.select', inspection: vectors.inspection });
    f.client.dispose();
    await expect(selected).rejects.toThrow('closed');
    const g = await fixture(inspecting);
    const pending = negotiatedSurface(g.client).execute(inspect);
    const request = g.sent.at(-1);
    if (request?.type !== 'surface.viewport.request') throw new Error('Missing inspect request.');
    expect(request.command).toEqual({ type: 'inspect', inspection: vectors.inspection });
    const center = { ...viewport.center };
    g.client.receive({
      protocol: request.protocol,
      version: request.version,
      session: request.session,
      requestId: request.requestId,
      type: 'surface.viewport.reply',
      address: request.address,
      revision: request.revision,
      status: 'applied',
      message: '',
      viewport: { ...viewport, center },
    });
    Object.assign(center, { x: 1 });
    const result = await pending;
    if (!('viewport' in result)) throw new Error('Missing native observation.');
    expect(result.viewport.center).toEqual(viewport.center);
    expect(Object.isFrozen(result.viewport.center)).toBe(true);
    g.client.dispose();
  } finally {
    f.client.dispose();
  }
});

test('host dispatch receives an immutable inspection copy and refuses it when undeclared', async () => {
  for (const declared of [inspecting, selectedPngCapabilities]) {
    const calls: CanvasSurfaceCommand[] = [];
    const sent: CanvasMessage[] = [];
    const server = new CanvasServer(session, {
      surface: {
        capabilities: declared,
        execute: async (command): Promise<CanvasSurfaceResult> => {
          calls.push(command);
          return command.type === 'surface.viewport.request'
            ? { status: 'applied', message: '', viewport }
            : { status: 'applied', message: '' };
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
      send: (message) => sent.push(message),
      report: (error) => {
        throw error;
      },
    });
    try {
      const solo = vectors.validRequests.filter(({ value }) => !('comparison' in value));
      for (const [index, vector] of solo.entries()) {
        expect(server.receive({ ...vector.value, requestId: index + 1 })).toBe(true);
      }
      for (let tick = 0; tick < 6; tick++) await Promise.resolve();
      if (declared === inspecting) {
        expect(calls).toHaveLength(solo.length);
        for (const command of calls) {
          const inspection =
            command.type === 'surface.select'
              ? command.inspection
              : command.type === 'surface.viewport.request' && command.command.type === 'inspect'
                ? command.command.inspection
                : undefined;
          expect(inspection).toBeDefined();
          expect(Object.isFrozen(inspection)).toBe(true);
          expect(Object.isFrozen(inspection?.center)).toBe(true);
          expect(Object.isFrozen(inspection?.span)).toBe(true);
        }
      } else {
        expect(calls).toEqual([]);
        expect(sent.map((message) => 'status' in message && message.status)).toEqual(
          solo.map(() => 'unavailable'),
        );
      }
    } finally {
      server.dispose();
    }
  }
});
