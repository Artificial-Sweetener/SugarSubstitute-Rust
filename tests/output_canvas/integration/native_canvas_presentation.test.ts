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
/** Exercise selection handoff with the actual shared surface command contract. */

import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import {
  connectCanvasSurface,
  selectedPngCapabilities,
} from '../../editor_integrations/support/canvas_surface.js';
import { NativeCanvasPresentation } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasPresentation.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type { CanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import type { CanvasCancellationSource } from '../../../integrations/contracts/CanvasHost.js';
import type {
  CanvasSurfaceCommand,
  CanvasSurfaceHost,
  CanvasSurfaceResult,
} from '../../../integrations/contracts/CanvasSurface.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type {
  CanvasSurfaceCommitRequest,
  CanvasSurfaceGeometryRequest,
  CanvasSurfacePrepareRequest,
  CanvasSurfaceReleaseRequest,
  CanvasSurfaceReply,
  CanvasSurfaceSelectRequest,
} from '../../../integrations/contracts/CanvasSurfaceProtocol.generated.js';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

type SurfaceRequest =
  | CanvasSurfacePrepareRequest
  | CanvasSurfaceGeometryRequest
  | CanvasSurfaceSelectRequest
  | CanvasSurfaceCommitRequest
  | CanvasSurfaceReleaseRequest;

const geometry: CanvasSurfaceGeometry = {
  coordinateSpace: 'client',
  units: 'css-pixels',
  origin: { x: 700, y: 100 },
  viewport: { width: 500, height: 600 },
  clip: { x: 700, y: 100, width: 500, height: 600 },
  physicalPixelsPerCssPixel: 1.5,
  visible: true,
  focused: false,
};

/** Hold actual operation completions so tests can replace the selected result while work is pending. */
function hostFixture() {
  const calls: CanvasSurfaceCommand[] = [];
  const pending: { resolve(result: CanvasSurfaceResult): void; cancelled(): boolean }[] = [];
  const host: CanvasSurfaceHost = {
    capabilities: selectedPngCapabilities,
    execute(command: CanvasSurfaceCommand, cancellation?: CanvasCancellationSource) {
      calls.push(command);
      return new Promise<CanvasSurfaceResult>((resolve) => {
        let cancelled = false;
        const stop = cancellation?.subscribe(() => {
          cancelled = true;
          resolve({ status: 'cancelled', message: '' });
        });
        pending.push({
          resolve: (result) => {
            stop?.();
            resolve(result);
          },
          cancelled: () => cancelled,
        });
      });
    },
  };
  return {
    host,
    calls,
    pending,
    respond(index: number, status: CanvasSurfaceResult['status']) {
      pending[index]!.resolve({ status, message: '' });
    },
  };
}

/** Retain actual CanvasClient traffic so presentation tests cross the cancellation transport. */
function clientHostFixture() {
  const sent: CanvasMessage[] = [];
  const reports: unknown[] = [];
  const client = new CanvasClient('native-presentation', {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: (error) => reports.push(error),
  });
  return {
    client,
    sent,
    reports,
    requests: (): SurfaceRequest[] => sent.filter(isSurfaceRequest),
    respond(request: SurfaceRequest, status: CanvasSurfaceReply['status']): boolean {
      return client.receive({
        protocol: request.protocol,
        version: request.version,
        session: request.session,
        requestId: request.requestId,
        type: 'surface.reply',
        address: request.address,
        revision: request.revision,
        status,
        message: '',
      });
    },
  };
}

/** Narrow shared client messages to operations that require a native acknowledgement. */
function isSurfaceRequest(message: CanvasMessage): message is SurfaceRequest {
  return (
    message.type === 'surface.prepare' ||
    message.type === 'surface.geometry' ||
    message.type === 'surface.select' ||
    message.type === 'surface.commit' ||
    message.type === 'surface.release'
  );
}

async function settle(): Promise<void> {
  for (let index = 0; index < 8; index++) await Promise.resolve();
}

function fixture() {
  const catalog = canvasCatalogFixture();
  let key = 0;
  const views = new CanvasViewRegistry(() => String(++key));
  const view = views.create(catalog.sessionId).viewId;
  const address = views.prepareAttachment(view, windowId('native'));
  views.commitAttachment(address);
  let host: CanvasSurfaceHost | null = null;
  const visible: boolean[] = [];
  const errors: unknown[] = [];
  const surface = new NativeCanvasPresentation({
    clock: new WorkflowTestClock(),
    preview: () => {},
    acceptsPreparation: () => false,
    prepared: () => {},
    address,
    host: () => host,
    accepts: () => views.accepts(address),
    visible: (value) => visible.push(value),
    status: () => {},
    viewport: () => {},
    report: (error) => errors.push(error),
  });
  return {
    surface,
    address,
    visible,
    errors,
    first: catalog.entries[0]!.items[0]!,
    second: catalog.entries[1]!.items[0]!,
    setHost: (value: CanvasSurfaceHost | null) => {
      host = value;
    },
  };
}

/** Establish one committed native allocation through the real shared client. */
async function revealClientSurface(
  presentation: ReturnType<typeof fixture>,
  native: ReturnType<typeof clientHostFixture>,
): Promise<void> {
  await connectCanvasSurface(native.client, native.sent);
  presentation.setHost(native.client.surface);
  presentation.surface.update(geometry, presentation.first);
  for (const [index, status] of ['ready', 'applied', 'applied', 'applied'].entries()) {
    const request = native.requests()[index];
    if (!request) throw new Error(`Surface request ${index} is missing.`);
    expect(native.respond(request, status as CanvasSurfaceReply['status'])).toBe(true);
    await settle();
  }
  expect(presentation.visible).toEqual([true]);
}

/** Complete release so fixture cleanup does not introduce another cancelled request. */
async function retireClientSurface(
  presentation: ReturnType<typeof fixture>,
  native: ReturnType<typeof clientHostFixture>,
): Promise<void> {
  presentation.surface.dispose();
  const release = native.requests().at(-1);
  if (!release || release.type !== 'surface.release')
    throw new Error('Surface release request is missing.');
  expect(native.respond(release, 'applied')).toBe(true);
  await settle();
  native.client.dispose();
}

test('prepares the newest selected output and reveals it only after native commit', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  expect(native.calls.map((call) => call.type)).toEqual(['surface.prepare']);
  f.surface.update(geometry, f.second);
  native.respond(0, 'ready');
  await settle();
  expect(native.calls[1]).toMatchObject({ type: 'surface.geometry', geometry });
  native.respond(1, 'applied');
  await settle();
  expect(native.calls[2]).toMatchObject({
    type: 'surface.select',
    itemId: f.second.id,
    file: f.second.file,
  });
  native.respond(2, 'applied');
  await settle();
  expect(native.calls[3]?.type).toBe('surface.commit');
  expect(f.visible).toEqual([]);
  native.respond(3, 'applied');
  await settle();
  expect(f.visible).toEqual([true]);
  expect(native.calls.map((call) => call.revision)).toEqual([1, 2, 3, 4]);
  expect(native.calls.every((call) => call.address === f.address)).toBe(true);
  expect(f.errors).toEqual([]);
  f.surface.dispose();
});

test('waits for a drawable clip before requesting an image from the native host', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update({ ...geometry, clip: { ...geometry.clip, width: 0, height: 0 } }, f.first);
  expect(native.calls).toEqual([]);
  f.surface.update(geometry, f.first);
  await settle();
  expect(native.calls.map((call) => call.type)).toEqual(['surface.prepare']);
  native.respond(0, 'ready');
  await settle();
  expect(native.calls[1]).toMatchObject({ type: 'surface.geometry', geometry });
  f.surface.dispose();
});

test('hides a committed surface for an empty clip and restores it when the clip returns', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  for (const [index, status] of ['ready', 'applied', 'applied', 'applied'].entries()) {
    native.respond(index, status as CanvasSurfaceResult['status']);
    await settle();
  }
  f.surface.update({ ...geometry, clip: { ...geometry.clip, width: 0, height: 0 } }, f.first);
  expect(native.calls[4]).toMatchObject({ type: 'surface.geometry', geometry: { visible: false } });
  native.respond(4, 'applied');
  await settle();
  f.surface.update(geometry, f.first);
  expect(native.calls[5]).toMatchObject({ type: 'surface.geometry', geometry });
  native.respond(5, 'applied');
  await settle();
  expect(f.visible).toEqual([true, false, true]);
  expect(native.calls.filter((call) => call.type === 'surface.select')).toHaveLength(1);
  f.surface.dispose();
});

test('switches the selected source without replacing the shared web controls', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  for (const [index, status] of ['ready', 'applied', 'applied', 'applied'].entries()) {
    native.respond(index, status as CanvasSurfaceResult['status']);
    await settle();
  }
  expect(f.visible).toEqual([true]);
  f.surface.update(geometry, f.second);
  expect(f.visible).toEqual([true]);
  expect(native.calls[4]).toMatchObject({ type: 'surface.select', itemId: f.second.id });
  native.respond(4, 'applied');
  await settle();
  expect(f.visible).toEqual([true]);
  expect(native.calls.filter((call) => call.type === 'surface.prepare')).toHaveLength(1);
  expect(native.calls.filter((call) => call.type === 'surface.commit')).toHaveLength(1);
  f.surface.dispose();
});

test('does not reveal a superseded selection that finishes after a newer tab was chosen', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  native.respond(0, 'ready');
  await settle();
  native.respond(1, 'applied');
  await settle();
  expect(native.calls[2]).toMatchObject({ type: 'surface.select', itemId: f.first.id });
  f.surface.update(geometry, f.second);
  native.respond(2, 'applied');
  await settle();
  expect(f.visible).toEqual([]);
  expect(native.calls[3]).toMatchObject({ type: 'surface.select', itemId: f.second.id });
  native.respond(3, 'applied');
  await settle();
  expect(native.calls[4]?.type).toBe('surface.commit');
  native.respond(4, 'applied');
  await settle();
  expect(f.visible).toEqual([true]);
  f.surface.dispose();
});

test('releases a disconnected host before using a replacement connection', async () => {
  const f = fixture();
  const first = hostFixture();
  const second = hostFixture();
  f.setHost(first.host);
  f.surface.update(geometry, f.first);
  f.setHost(second.host);
  f.surface.update(geometry, f.first);
  expect(first.pending[0]!.cancelled()).toBe(true);
  expect(first.calls[1]).toMatchObject({ type: 'surface.release', address: f.address });
  await settle();
  expect(second.calls).toHaveLength(0);
  first.respond(1, 'applied');
  await settle();
  expect(second.calls[0]).toMatchObject({ type: 'surface.prepare', address: f.address });
  first.respond(0, 'ready');
  await settle();
  expect(f.visible).toEqual([]);
  expect(second.calls).toHaveLength(1);
  f.surface.dispose();
});

test('keeps the native presentation hidden when the host declines the attachment', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  native.respond(0, 'unavailable');
  await settle();
  expect(f.visible).toEqual([]);
  expect(native.calls.map((call) => call.type)).toEqual(['surface.prepare']);
  expect(f.errors).toEqual([]);
  f.surface.update(geometry, f.first);
  await settle();
  expect(native.calls).toHaveLength(1);
  f.surface.dispose();
});

test('reconciles stale geometry on an explicit refresh after a sibling attachment releases', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  for (const [index, status] of ['ready', 'applied', 'applied', 'applied'].entries()) {
    native.respond(index, status as CanvasSurfaceResult['status']);
    await settle();
  }
  const resized = { ...geometry, viewport: { width: 400, height: 600 } };
  f.surface.update(resized, f.first);
  native.respond(4, 'stale');
  await settle();
  expect(native.calls).toHaveLength(5);
  expect(f.visible).toEqual([true, false]);
  f.surface.update(resized, f.first);
  expect(native.calls[5]).toMatchObject({ type: 'surface.geometry', geometry: resized });
  native.respond(5, 'applied');
  await settle();
  expect(f.visible).toEqual([true, false, true]);
  const released = f.surface.dispose();
  native.respond(6, 'applied');
  await released;
});

test('reveals native pixels only after the surface accepts the current geometry', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  for (const [index, status] of ['ready', 'applied', 'applied', 'applied'].entries()) {
    native.respond(index, status as CanvasSurfaceResult['status']);
    await settle();
  }
  const resized = { ...geometry, viewport: { width: 400, height: 600 } };
  f.surface.update(resized, f.first);
  expect(f.visible).toEqual([true, false]);
  expect(native.calls[4]).toMatchObject({ type: 'surface.geometry', geometry: resized });
  native.respond(4, 'applied');
  await settle();
  expect(f.visible).toEqual([true, false, true]);
  f.surface.update(resized, null);
  expect(f.visible).toEqual([true, false, true, false]);
  expect(native.calls[5]).toMatchObject({
    type: 'surface.geometry',
    geometry: { visible: false, clip: { x: 0, y: 0, width: 0, height: 0 } },
  });
  native.respond(5, 'applied');
  await settle();
  f.surface.dispose();
});

test('cancels a pending attachment and releases its exact address before late readiness', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  const released = f.surface.dispose();
  let settled = false;
  void released.then(() => {
    settled = true;
  });
  expect(f.surface.dispose()).toBe(released);
  expect(native.pending[0]!.cancelled()).toBe(true);
  expect(native.calls[1]).toEqual({ type: 'surface.release', address: f.address, revision: 2 });
  native.respond(0, 'ready');
  await settle();
  expect(f.visible).toEqual([]);
  expect(native.calls).toHaveLength(2);
  expect(settled).toBe(false);
  native.respond(1, 'applied');
  await released;
  expect(settled).toBe(true);
});

test('rejects disposal when native release fails and fences replacement preparation', async () => {
  const f = fixture();
  const first = hostFixture();
  const second = hostFixture();
  f.setHost(first.host);
  f.surface.update(geometry, f.first);
  f.setHost(second.host);
  f.surface.update(geometry, f.first);
  first.respond(1, 'error');
  await settle();
  expect(second.calls).toHaveLength(0);
  expect(f.errors).toHaveLength(1);
  await expect(f.surface.dispose()).rejects.toThrow('Native canvas release failed.');
});

test('replaces pending geometry when another resize makes its presentation impossible', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  for (const [index, status] of ['ready', 'applied', 'applied', 'applied'].entries()) {
    native.respond(index, status as CanvasSurfaceResult['status']);
    await settle();
  }
  const intermediate = { ...geometry, viewport: { width: 520, height: 600 } };
  const final = { ...geometry, viewport: { width: 600, height: 700 } };
  f.surface.update(intermediate, f.first);
  expect(native.calls[4]).toMatchObject({ type: 'surface.geometry', geometry: intermediate });
  f.surface.update(final, f.first);
  expect(native.pending[4]!.cancelled()).toBe(true);
  await settle();
  expect(native.calls[5]).toMatchObject({ type: 'surface.geometry', geometry: final });
  native.respond(4, 'applied');
  await settle();
  expect(f.visible).toEqual([true, false]);
  native.respond(5, 'applied');
  await settle();
  expect(f.visible).toEqual([true, false, true]);
  expect(native.calls.filter((call) => call.type === 'surface.select')).toHaveLength(1);
  expect(f.errors).toEqual([]);
  f.surface.dispose();
});

test('coalesces fractional geometry churn until native cancellation is acknowledged', async () => {
  const f = fixture();
  const native = clientHostFixture();
  await revealClientSurface(f, native);
  let latest = geometry;
  for (let index = 0; index < 1400; index++) {
    const x = geometry.origin.x + (index % 2 === 0 ? 1 / 64 : 0);
    latest = { ...geometry, origin: { ...geometry.origin, x }, clip: { ...geometry.clip, x } };
    f.surface.update(latest, f.first);
  }
  await settle();
  const pending = native.requests()[4];
  if (!pending || pending.type !== 'surface.geometry')
    throw new Error('Pending geometry request is missing.');
  expect(native.sent.slice(6).map(({ type, requestId }) => ({ type, requestId }))).toEqual([
    { type: 'surface.geometry', requestId: pending.requestId },
    { type: 'cancel', requestId: pending.requestId },
  ]);
  expect(native.respond(pending, 'cancelled')).toBe(true);
  await settle();
  const replacement = native.requests()[5];
  expect(replacement).toMatchObject({ type: 'surface.geometry', geometry: latest });
  if (!replacement) throw new Error('Replacement geometry request is missing.');
  expect(native.respond(replacement, 'applied')).toBe(true);
  await settle();
  expect(f.visible).toEqual([true, false, true]);
  expect(f.errors).toEqual([]);
  expect(native.reports).toEqual([]);
  await retireClientSurface(f, native);
});

test('reconciles newest geometry when native application wins the cancellation race', async () => {
  const f = fixture();
  const native = clientHostFixture();
  await revealClientSurface(f, native);
  const intermediate = { ...geometry, viewport: { width: 520, height: 600 } };
  const final = { ...geometry, viewport: { width: 600, height: 700 } };
  f.surface.update(intermediate, f.first);
  f.surface.update(final, f.first);
  await settle();
  const pending = native.requests()[4];
  if (!pending || pending.type !== 'surface.geometry')
    throw new Error('Pending geometry request is missing.');
  expect(native.sent.slice(6).map(({ type, requestId }) => ({ type, requestId }))).toEqual([
    { type: 'surface.geometry', requestId: pending.requestId },
    { type: 'cancel', requestId: pending.requestId },
  ]);
  expect(native.respond(pending, 'applied')).toBe(true);
  await settle();
  const replacement = native.requests()[5];
  expect(replacement).toMatchObject({ type: 'surface.geometry', geometry: final });
  if (!replacement) throw new Error('Replacement geometry request is missing.');
  expect(native.respond(replacement, 'applied')).toBe(true);
  await settle();
  expect(f.visible).toEqual([true, false, true]);
  expect(f.errors).toEqual([]);
  expect(native.reports).toEqual([]);
  await retireClientSurface(f, native);
});

test('reasserts the last accepted bounds when a pending resize is reversed', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  for (const [index, status] of ['ready', 'applied', 'applied', 'applied'].entries()) {
    native.respond(index, status as CanvasSurfaceResult['status']);
    await settle();
  }
  f.surface.update({ ...geometry, viewport: { width: 600, height: 700 } }, f.first);
  f.surface.update(geometry, f.first);
  await settle();
  expect(native.pending[4]!.cancelled()).toBe(true);
  expect(native.calls[5]).toMatchObject({ type: 'surface.geometry', geometry });
  expect(f.visible).toEqual([true, false]);
  native.respond(5, 'applied');
  await settle();
  expect(f.visible).toEqual([true, false, true]);
  f.surface.dispose();
});

test('reconciles new bounds before retrying an unacknowledged first commit', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  for (const [index, status] of ['ready', 'applied', 'applied'].entries()) {
    native.respond(index, status as CanvasSurfaceResult['status']);
    await settle();
  }
  expect(native.calls[3]?.type).toBe('surface.commit');
  const resized = { ...geometry, viewport: { width: 600, height: 700 } };
  f.surface.update(resized, f.first);
  expect(native.pending[3]!.cancelled()).toBe(true);
  await settle();
  expect(native.calls[4]).toMatchObject({ type: 'surface.geometry', geometry: resized });
  native.respond(4, 'applied');
  await settle();
  expect(native.calls[5]?.type).toBe('surface.commit');
  expect(f.visible).toEqual([]);
  native.respond(5, 'applied');
  await settle();
  expect(f.visible).toEqual([true]);
  f.surface.dispose();
});

test('reselects the prior output after cancelling a pending source switch', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  for (const [index, status] of ['ready', 'applied', 'applied', 'applied'].entries()) {
    native.respond(index, status as CanvasSurfaceResult['status']);
    await settle();
  }
  f.surface.update(geometry, f.second);
  f.surface.update(geometry, f.first);
  await settle();
  expect(native.pending[4]!.cancelled()).toBe(true);
  expect(native.calls[5]).toMatchObject({ type: 'surface.select', itemId: f.first.id });
  expect(f.visible).toEqual([true]);
  native.respond(5, 'applied');
  await settle();
  expect(f.visible).toEqual([true]);
  f.surface.dispose();
});

test('hides the last native allocation if the web geometry disappears', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, f.first);
  for (const [index, status] of ['ready', 'applied', 'applied', 'applied'].entries()) {
    native.respond(index, status as CanvasSurfaceResult['status']);
    await settle();
  }
  f.surface.update(null, null);
  expect(f.visible).toEqual([true, false]);
  expect(native.calls[4]).toMatchObject({
    type: 'surface.geometry',
    geometry: { origin: geometry.origin, visible: false },
  });
  native.respond(4, 'applied');
  await settle();
  f.surface.dispose();
});

test('does not present unsupported media through the native surface', async () => {
  const f = fixture();
  const native = hostFixture();
  f.setHost(native.host);
  f.surface.update(geometry, { ...f.first, mediaKind: 'video' });
  expect(native.calls).toHaveLength(0);
  await settle();
  f.surface.update(geometry, f.first);
  for (const [index, status] of ['ready', 'applied', 'applied', 'applied'].entries()) {
    native.respond(index, status as CanvasSurfaceResult['status']);
    await settle();
  }
  expect(f.visible).toEqual([true]);
  f.surface.dispose();
});

test('keeps acknowledged pixels while a replacement loads and hides them when it fails', async () => {
  const f = fixture();
  const native = clientHostFixture();
  await revealClientSurface(f, native);
  f.surface.update(geometry, f.second);
  expect(f.visible).toEqual([true]);
  const selection = native.requests()[4];
  if (!selection || selection.type !== 'surface.select')
    throw new Error('Replacement selection is missing.');
  expect(native.respond(selection, 'error')).toBe(true);
  await settle();
  // Like a browser load error, the failed result never shows the previous image.
  expect(f.visible).toEqual([true, false]);
  expect(f.errors).toHaveLength(1);
  f.setHost(null);
  f.surface.update(geometry, f.second);
  expect(f.visible).toEqual([true, false]);
  const release = native.requests().at(-1);
  if (!release || release.type !== 'surface.release')
    throw new Error('Retired host release is missing.');
  expect(native.respond(release, 'applied')).toBe(true);
  await settle();
  f.surface.dispose();
  native.client.dispose();
});
