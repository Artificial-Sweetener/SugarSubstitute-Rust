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
/** Prove external overlay commands fail closed before reaching a view authority. */

import { decodeCanvasCommandRequest } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasCommandDecoder.js';
import type { CanvasCommand } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { canvasSnapshot } from '../support/CanvasPortFixture.js';

const address = canvasSnapshot('first').address;
const authority = {
  origin: 'https://substitute.example',
  address,
  capabilities: {
    schema: 1 as const,
    commands: [
      'reveal',
      'fit',
      'native',
      'select',
      'select-source',
      'select-preview',
      'batch',
      'follow',
      'overview',
      'link',
      'zoom',
      'pan',
      'comparison',
      'select-comparison',
      'wipe',
      'swap',
      'retry',
      'admit-large-image',
      'open-media',
    ] satisfies CanvasCommand['type'][],
  },
};

/** Build an untrusted transport value without weakening its boundary type. */
function request(command: unknown): unknown {
  return { schema: 1, address, requestId: 1, expectedRevision: 0, command };
}

test.each<CanvasCommand>([
  {
    type: 'reveal',
    target: {
      sessionId: 'workflow:acceptance',
      generation: 1,
      sourceId: 'source:7:42',
      runOrder: 2,
      itemId: 'result:exact-final',
      batchIndex: 2,
    },
  },
  { type: 'fit' },
  { type: 'native' },
  { type: 'swap' },
  { type: 'retry' },
  { type: 'admit-large-image' },
  { type: 'select', selection: { sourceId: 'source:7:42', batchIndex: 3 } },
  { type: 'select-source', selection: { sourceId: 'source:7:42', batchIndex: 3 } },
  { type: 'select-preview', sourceId: 'source:7:42' },
  { type: 'select-comparison', selection: { sourceId: 'source:42', batchIndex: 0 } },
  { type: 'comparison', enabled: false },
  { type: 'batch', index: 3 },
  { type: 'follow', mode: 'auto' },
  { type: 'overview', enabled: true },
  { type: 'link', enabled: false },
  { type: 'zoom', factor: 1.25 },
  { type: 'pan', x: -0.1, y: 0.1, units: 'viewport' },
  { type: 'wipe', orientation: 'horizontal', split: 0.3 },
  { type: 'open-media', target: 'secondary' },
])('decodes a bounded supported intent %#', (command) => {
  const external = request({ ...command, extra: { untrusted: true } });
  const decoded = decodeCanvasCommandRequest(external, authority.origin, authority);
  expect(decoded?.command).toEqual(command);
  expect(decoded?.address).toBe(address);
  expect(Object.isFrozen(decoded)).toBe(true);
  expect(Object.isFrozen(decoded?.command)).toBe(true);
});

test.each([
  {
    type: 'reveal',
    target: {
      sessionId: 'other:acceptance',
      generation: 1,
      sourceId: 'source:7:42',
      runOrder: 2,
      itemId: 'result:exact-final',
      batchIndex: 2,
    },
  },
  {
    type: 'reveal',
    target: {
      sessionId: 'workflow:acceptance',
      generation: 1,
      sourceId: 'source:7:42',
      runOrder: 2,
      itemId: 'blob:preview',
      batchIndex: 2,
    },
  },
  {
    type: 'reveal',
    target: {
      sessionId: 'workflow:acceptance',
      generation: 1,
      sourceId: 'source:7:42',
      runOrder: 2,
      itemId: 'result:exact-final',
      batchIndex: -1,
    },
  },
  { type: 'unknown' },
  { type: 'batch', index: -1 },
  { type: 'batch', index: 1.2 },
  { type: 'zoom', factor: NaN },
  { type: 'zoom', factor: Infinity },
  { type: 'zoom', factor: 0 },
  { type: 'pan', x: 1000, y: 0, units: 'viewport' },
  { type: 'pan', x: 0, y: 0, units: 'physical' },
  { type: 'wipe', orientation: 'diagonal', split: 0.5 },
  { type: 'wipe', orientation: 'vertical', split: 2 },
  { type: 'select', selection: { sourceId: '7', batchIndex: 0 } },
  { type: 'select-source', selection: { sourceId: 'source:7', batchIndex: -1 } },
  { type: 'select-preview', sourceId: '7' },
  { type: 'select-preview' },
  { type: 'select', selection: { sourceId: `source:${'x'.repeat(4096)}`, batchIndex: 0 } },
  { type: 'select-comparison' },
  { type: 'follow', mode: 'random' },
  { type: 'overview', enabled: 'true' },
  { type: 'open-media', target: 'unknown' },
  null,
])('rejects malformed external parameters %#', (command) => {
  expect(decodeCanvasCommandRequest(request(command), authority.origin, authority)).toBeNull();
});

test('rejects unauthorized origins, unavailable capabilities and stale attachment generations', () => {
  const value = { schema: 1, address, requestId: 1, expectedRevision: 0, command: { type: 'fit' } };
  expect(decodeCanvasCommandRequest(value, 'https://other.example', authority)).toBeNull();
  expect(decodeCanvasCommandRequest(value, 'null', { ...authority, origin: 'null' })).toBeNull();
  expect(decodeCanvasCommandRequest(value, '', { ...authority, origin: '' })).toBeNull();
  expect(
    decodeCanvasCommandRequest(value, authority.origin, {
      ...authority,
      capabilities: { schema: 1, commands: [] },
    }),
  ).toBeNull();
  expect(
    decodeCanvasCommandRequest(
      { ...value, address: { ...address, generation: 0 } },
      authority.origin,
      authority,
    ),
  ).toBeNull();
  expect(
    decodeCanvasCommandRequest({ ...value, schema: 2 }, authority.origin, authority),
  ).toBeNull();
  expect(
    decodeCanvasCommandRequest({ ...value, requestId: 0 }, authority.origin, authority),
  ).toBeNull();
  expect(
    decodeCanvasCommandRequest({ ...value, expectedRevision: -1 }, authority.origin, authority),
  ).toBeNull();
});
