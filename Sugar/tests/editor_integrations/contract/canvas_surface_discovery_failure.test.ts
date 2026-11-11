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
/** Prove an unreadable discovery reply ends discovery at once with a typed failure. */
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import {
  CanvasSurfaceDiscoveryFailure,
  type CanvasSurfaceDiscoveryFailureReason,
} from '../../../integrations/shared/CanvasSurfaceDiscovery.js';
import type {
  CanvasContext,
  CanvasMessage,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { selectedPngCapabilities } from '../support/canvas_surface.js';

const session = 'surface-discovery-failure';
const context: CanvasContext = {
  canvas: {
    id: 'native',
    label: 'Native',
    capabilities: { displayOutput: true, insertImageLayer: false, positionImageLayer: false },
  },
  document: null,
};

/** Hold deadlines without firing them, so any failure observed is not a timeout. */
function fixture() {
  const sent: CanvasMessage[] = [];
  const deadlines: (() => void)[] = [];
  const stops = jest.fn();
  const report = jest.fn<(error: unknown) => void>();
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: (_duration, callback) => {
      deadlines.push(callback);
      return stops;
    },
    report,
  });
  return { client, sent, deadlines, stops, report };
}

/** Answer the context read and return the discovery request it starts. */
async function discovery(f: ReturnType<typeof fixture>) {
  const pending = f.client.readContext();
  const request = f.sent[0];
  if (request?.type !== 'context.request') throw new Error('Missing context request.');
  f.client.receive({ ...request, type: 'context.reply', context });
  await Promise.resolve();
  const discover = f.sent[1];
  if (discover?.type !== 'surface.capabilities.request')
    throw new Error('Missing discovery request.');
  return { pending, discover };
}

/** Build a reply envelope for the outstanding discovery. */
function reply(discover: { requestId: number }, patch: Record<string, unknown>) {
  return {
    protocol: 'sugarcubes.canvas',
    version: 3,
    session,
    requestId: discover.requestId,
    type: 'surface.capabilities.reply',
    status: 'available',
    capabilities: selectedPngCapabilities,
    message: '',
    ...patch,
  };
}

/** Read the typed reason the page reported for its absent surface. */
function reportedReason(report: jest.Mock<(error: unknown) => void>) {
  const error = report.mock.calls[0]?.[0];
  return error instanceof CanvasSurfaceDiscoveryFailure ? error.reason : null;
}

test.each([
  ['a schema newer than requested', { capabilities: { ...selectedPngCapabilities, schema: 4 } }],
  [
    'an unknown image format',
    { capabilities: { ...selectedPngCapabilities, imageFormats: ['bmp'] } },
  ],
  ['an undeclared member', { capabilities: { ...selectedPngCapabilities, extra: true } }],
  ['missing capabilities', { capabilities: undefined }],
  ['an unknown status', { status: 'ready' }],
])('ends discovery at once on a reply with %s', async (_label, patch) => {
  const f = fixture();
  const { pending, discover } = await discovery(f);
  expect(f.client.receive(reply(discover, patch))).toBe(false);
  await pending;
  expect(f.client.surface).toBeNull();
  expect(reportedReason(f.report)).toBe<CanvasSurfaceDiscoveryFailureReason>('invalid-reply');
  expect(f.report).toHaveBeenCalledTimes(1);
  // The discovery deadline was retired without ever firing.
  expect(f.stops).toHaveBeenCalled();
  expect(f.sent.map(({ type }) => type)).toEqual([
    'context.request',
    'surface.capabilities.request',
  ]);
  // A late valid answer for the ended request is refused.
  expect(f.client.receive(reply(discover, {}))).toBe(false);
  expect(f.client.surface).toBeNull();
  f.client.dispose();
});

test.each([
  ['another session', { session: 'surface-discovery-other' }],
  ['an unknown request', { requestId: 999 }],
  ['another message type', { type: 'surface.reply' }],
  ['another protocol', { protocol: 'sugarcubes.panel' }],
])('ignores an invalid message addressed to %s and keeps discovering', async (_label, patch) => {
  const f = fixture();
  const { pending, discover } = await discovery(f);
  expect(
    f.client.receive(
      reply(discover, { ...patch, capabilities: { ...selectedPngCapabilities, schema: 4 } }),
    ),
  ).toBe(false);
  expect(f.report).not.toHaveBeenCalled();
  expect(f.client.receive(reply(discover, {}))).toBe(true);
  await pending;
  expect(f.client.surface?.capabilities.imageFormats).toEqual(['png']);
  f.client.dispose();
});

test('a discovery deadline reports a typed timeout', async () => {
  const f = fixture();
  const { pending } = await discovery(f);
  f.deadlines[1]?.();
  await pending;
  expect(f.client.surface).toBeNull();
  expect(reportedReason(f.report)).toBe<CanvasSurfaceDiscoveryFailureReason>('timed-out');
  f.client.dispose();
});

test('a valid reply declaring capabilities while unavailable fails with its own reason', async () => {
  const f = fixture();
  const { pending, discover } = await discovery(f);
  expect(f.client.receive(reply(discover, { status: 'unavailable' }))).toBe(true);
  await pending;
  expect(f.client.surface).toBeNull();
  expect(reportedReason(f.report)).toBe<CanvasSurfaceDiscoveryFailureReason>(
    'declared-while-unavailable',
  );
  f.client.dispose();
});
