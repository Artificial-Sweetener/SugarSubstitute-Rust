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
/** Qualify page-published gesture occlusions through shared vectors and the real canvas session. */
import { readFileSync } from 'node:fs';
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import {
  isCanvasSurfaceGeometry,
  isCanvasSurfaceGeometryRequest,
  isCanvasSurfaceOcclusion,
} from '../../../integrations/shared/CanvasSurfaceValidation.generated.js';
import {
  CANVAS_SURFACE_OCCLUSIONS,
  CANVAS_SURFACE_OCCLUSION_EXTENT,
} from '../../../integrations/shared/CanvasSurfaceOcclusion.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfaceResult,
} from '../../../integrations/contracts/CanvasSurface.js';
import { negotiatedSurface, selectedPngCapabilities } from '../support/canvas_surface.js';

interface Vector {
  readonly name: string;
  readonly value: Record<string, unknown>;
}
const vectors: {
  maxOcclusions: number;
  maxExtent: number;
  validRequests: Vector[];
  invalidRequests: Vector[];
} = JSON.parse(
  readFileSync(
    new URL(
      '../../../integrations/contracts/canvas_surface_occlusion_vectors.json',
      import.meta.url,
    ),
    'utf8',
  ),
);
const session = 'surface-occlusion-session';

/** Read one vector's geometry request, which every valid vector carries. */
function geometryRequest(vector: Vector) {
  const value = vector.value;
  if (!isCanvasSurfaceGeometryRequest(value)) throw new Error(`Invalid vector: ${vector.name}`);
  return value;
}

test('pins the shared occlusion bounds that the native host mirrors', () => {
  expect(CANVAS_SURFACE_OCCLUSIONS).toBe(vectors.maxOcclusions);
  expect(CANVAS_SURFACE_OCCLUSION_EXTENT).toBe(vectors.maxExtent);
});

test.each(vectors.validRequests)('admits occlusion vector: $name', ({ value }) => {
  expect(isCanvasMessage(value)).toBe(true);
  expect(isCanvasSurfaceGeometryRequest(value)).toBe(true);
});

test.each(vectors.invalidRequests)('refuses malformed occlusion vector: $name', ({ value }) => {
  expect(isCanvasMessage(value)).toBe(false);
  expect(isCanvasSurfaceGeometryRequest(value)).toBe(false);
});

test('refuses non-finite rect members that JSON vectors cannot carry', () => {
  const rect = { x: 1, y: 1, width: 1, height: 1 };
  for (const number of [Number.NaN, Infinity, -Infinity])
    for (const key of ['x', 'y', 'width', 'height'] as const)
      expect(isCanvasSurfaceOcclusion({ ...rect, [key]: number })).toBe(false);
  const { geometry } = geometryRequest(vectors.validRequests[0]!);
  expect(isCanvasSurfaceGeometry({ ...geometry, occlusions: [rect] })).toBe(true);
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

test.each([true, false])(
  'carries immutable occlusions only to a host declaring input: %s',
  async (input) => {
    const f = fixture({ ...selectedPngCapabilities, input });
    try {
      await f.client.readContext();
      const routed = geometryRequest(vectors.validRequests[2]!);
      const plain = geometryRequest(vectors.validRequests[0]!);
      const command = { type: 'surface.geometry', address: routed.address, revision: 4 } as const;
      const result = await negotiatedSurface(f.client).execute({
        ...command,
        geometry: routed.geometry,
      });
      expect(result.status).toBe(input ? 'applied' : 'unavailable');
      // Geometry without routing members remains admissible for every host.
      expect(
        (
          await negotiatedSurface(f.client).execute({
            ...command,
            revision: 5,
            geometry: plain.geometry,
          })
        ).status,
      ).toBe('applied');
      const calls = f.execute.mock.calls.map(([call]) => call);
      expect(calls.map((call) => call.type === 'surface.geometry' && call.geometry)).toEqual(
        input ? [routed.geometry, plain.geometry] : [plain.geometry],
      );
      for (const call of calls) {
        if (call.type !== 'surface.geometry') throw new Error('Missing geometry.');
        if (call.geometry.occlusions) {
          expect(Object.isFrozen(call.geometry.occlusions)).toBe(true);
          expect(call.geometry.occlusions.every((rect) => Object.isFrozen(rect))).toBe(true);
        }
      }
      expect(f.errors).toEqual([]);
    } finally {
      f.client.dispose();
      f.server.dispose();
    }
  },
);

test('replies unavailable at the receiving surface owner for occlusions sent to a plain host', async () => {
  const replies: CanvasMessage[] = [];
  const execute = jest.fn(
    async (): Promise<CanvasSurfaceResult> => ({ status: 'applied', message: '' }),
  );
  const server = new CanvasServer(session, {
    surface: { capabilities: selectedPngCapabilities, execute },
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
    report: () => {},
  });
  server.receive(vectors.validRequests[5]!.value);
  for (let turn = 0; turn < 8; turn++) await Promise.resolve();
  expect(replies).toEqual([
    expect.objectContaining({ type: 'surface.reply', requestId: 5, status: 'unavailable' }),
  ]);
  expect(execute).not.toHaveBeenCalled();
  server.dispose();
});
