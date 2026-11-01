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
/** Prove native preview byte custody, fair operation credit and truthful displayed state. */
import { afterEach, expect, test } from '@jest/globals';
import { NativeCanvasPresentation } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasPresentation.js';
import type { NativeCanvasPresentationStatus } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasStatus.js';
import type { WorkflowPreview } from '../../../frontend/comfyui/ui/execution/domain/WorkflowPreview.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type {
  CanvasSurfacePresentationDisplay,
  CanvasSurfaceViewportState,
} from '../../../integrations/contracts/CanvasSurface.js';
import { decodeCanvasBytes } from '../../../integrations/shared/CanvasByteEncoding.js';
import { canvasSnapshot, canvasNavigationScope } from '../support/CanvasPortFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import {
  NativePreviewBlob,
  nativePreviewHost,
  previewGeometry,
  finalViewport,
  previewViewport,
  settleNativePreview as settle,
} from '../support/NativeCanvasPreviewFixture.js';

const retirements: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const retire of retirements.splice(0)) await retire();
});

/** Compose real presentation state with only the native host, clock and encoded reads controlled. */
function fixture(supported = true) {
  const host = nativePreviewHost(supported);
  const catalog = canvasCatalogFixture();
  const clock = new WorkflowTestClock();
  const address = canvasSnapshot('native-preview').address;
  const previews: boolean[] = [];
  const viewports: (CanvasSurfaceViewportState | null)[] = [];
  const statuses: NativeCanvasPresentationStatus[] = [];
  const errors: unknown[] = [];
  const visible: boolean[] = [];
  let accepts = true;
  const presentation = new NativeCanvasPresentation({
    address,
    clock,
    host: () => host.host,
    accepts: () => accepts,
    acceptsPreparation: () => false,
    prepared: () => {},
    visible: (value) => visible.push(value),
    preview: (value) => previews.push(value),
    viewport: (value) => viewports.push(value),
    status: (value) => statuses.push(value),
    report: (error) => errors.push(error),
  });
  retirements.push(async () => {
    const retired = presentation.dispose();
    for (const [index, pending] of host.pending.entries()) {
      if (!pending.settled)
        host.respond(
          index,
          host.calls[index]?.type === 'surface.release' ? 'applied' : 'cancelled',
        );
    }
    await retired;
  });
  const item = catalog.entries[0]!.items[0]!;
  const preview = (sequence = 1): WorkflowPreview => ({
    sessionId: catalog.result.context.sessionId,
    generation: catalog.generation,
    promptId: 'prompt:preview',
    sourceId: item.provenance.sourceId,
    runOrder: 1,
    sequence,
    token: `preview:${sequence}`,
    dimensions: null,
  });
  return {
    ...host,
    clock,
    address,
    presentation,
    previews,
    viewports,
    statuses,
    errors,
    visible,
    item,
    second: catalog.entries[1]!.items[0]!,
    preview,
    deactivate: () => {
      accepts = false;
    },
    scope: () => canvasNavigationScope(),
  };
}

/** Establish the existing final-selection contract without granting preview presentation. */
async function commit(
  f: ReturnType<typeof fixture>,
  item: WorkflowOutputItem | null = f.item,
): Promise<void> {
  f.presentation.update(previewGeometry, item, f.preview());
  f.ready('observing');
  await settle();
  for (let index = 0; index < (item ? 4 : 3); index++) {
    f.respond(index, index === 0 ? 'ready' : 'applied');
    await settle();
  }
  if (item) {
    f.respond(4, 'applied', finalViewport);
    await settle();
  }
}

/** Read the actual begin command's binding-assigned revision, never a later camera revision. */
function begin(f: ReturnType<typeof fixture>, token = 'preview:1') {
  const command = f.calls.find(
    (call) => call.type === 'surface.preview.begin' && call.preview.token === token,
  );
  if (!command || command.type !== 'surface.preview.begin')
    throw new Error('Preview begin is missing.');
  return command;
}

/** Deliver one actual lease-correlated observation from the external native presentation owner. */
function observe(
  f: ReturnType<typeof fixture>,
  command: ReturnType<typeof begin>,
  displayed: CanvasSurfacePresentationDisplay,
  status: 'presented' | 'failed' | 'cancelled' = 'presented',
  sequence = 1,
): void {
  f.emit({
    address: f.address,
    token: command.preview.token,
    leaseRevision: command.revision,
    sequence,
    status,
    displayed,
    message: status === 'failed' ? 'Native candidate failed.' : '',
  });
}

/** Transfer one small encoded frame while keeping decoder acceptance explicitly external. */
async function transfer(
  f: ReturnType<typeof fixture>,
  blob = new NativePreviewBlob(),
  preview = f.preview(),
): Promise<ReturnType<typeof begin>> {
  f.presentation.update(previewGeometry, f.item, preview);
  const offset = f.calls.length;
  f.presentation.receive(preview, blob);
  expect(f.calls[offset]?.type).toBe('surface.preview.begin');
  f.respond(offset);
  await settle();
  blob.release();
  await settle();
  expect(f.calls[offset + 1]?.type).toBe('surface.preview.chunk');
  f.respond(offset + 1);
  await settle();
  expect(f.calls[offset + 2]?.type).toBe('surface.preview.end');
  f.respond(offset + 2);
  await settle();
  return begin(f, preview.token);
}

test('requires an accepted watch before consuming bytes, while the final remains interactive', async () => {
  const f = fixture();
  f.presentation.update(previewGeometry, f.item, f.preview());
  for (let index = 0; index < 4; index++) {
    f.respond(index, index ? 'applied' : 'ready');
    await settle();
  }
  f.respond(4, 'applied', finalViewport);
  await settle();
  const refused = new NativePreviewBlob();
  expect(f.presentation.wants()).toBe(false);
  f.presentation.receive(f.preview(), refused);
  expect(refused.reads).toHaveLength(0);
  expect(f.calls).toHaveLength(5);
  const navigation = f.presentation.viewport({ type: 'native' }, f.scope());
  f.respond(5, 'applied', finalViewport);
  expect(await navigation).toBe(true);
  f.ready('observing');
  await settle();
  expect(f.presentation.wants()).toBe(true);
  f.presentation.receive(f.preview(), new NativePreviewBlob());
  expect(f.calls[6]?.type).toBe('surface.preview.begin');
});

test('releases each custody ticket so final navigation interleaves before and between exact byte chunks', async () => {
  const f = fixture();
  await commit(f);
  const blob = new NativePreviewBlob(8195);
  f.presentation.receive(f.preview(), blob);
  const lease = begin(f);
  const navigation = f.presentation.viewport({ type: 'native' }, f.scope());
  f.respond(5);
  await settle();
  expect(f.calls[6]?.type).toBe('surface.viewport.request');
  f.respond(6, 'applied', finalViewport);
  expect(await navigation).toBe(true);
  await settle();
  expect(blob.reads[0]).toMatchObject({ start: 0, end: 8192 });
  const pan = f.presentation.viewport({ type: 'pan', x: 0.1, y: 0, units: 'viewport' }, f.scope());
  expect(f.calls[7]?.type).toBe('surface.viewport.request');
  blob.release();
  await settle();
  expect(f.calls).toHaveLength(8);
  f.respond(7, 'applied', finalViewport);
  expect(await pan).toBe(true);
  await settle();
  const first = f.calls[8];
  if (first?.type !== 'surface.preview.chunk') throw new Error('First chunk is missing.');
  expect(first.leaseRevision).toBe(lease.revision);
  expect(first.revision).toBeGreaterThan(first.leaseRevision);
  expect(decodeCanvasBytes(first.data, 8192)).toEqual(blob.encoded.subarray(0, 8192));
  const zoom = f.presentation.viewport({ type: 'zoom', factor: 2 }, f.scope());
  f.respond(8);
  await settle();
  expect(f.calls[9]?.type).toBe('surface.viewport.request');
  f.respond(9, 'applied', finalViewport);
  expect(await zoom).toBe(true);
  await settle();
  expect(blob.reads[1]).toMatchObject({ start: 8192, end: 8195 });
  blob.release(1);
  await settle();
  expect(f.calls[10]).toMatchObject({
    type: 'surface.preview.chunk',
    leaseRevision: lease.revision,
    offset: 8192,
  });
  f.respond(10);
  await settle();
  f.respond(11);
  await settle();
  expect(f.previews).not.toContain(true);
  expect(f.presentation.diagnostics().estimatedBytes).toBe(0);
});

test('locks Fit only after displayed pixels, fences obsolete camera completion and restores the accepted final camera', async () => {
  const f = fixture();
  await commit(f);
  const lease = await transfer(f);
  expect(f.previews).not.toContain(true);
  const navigation = f.presentation.viewport({ type: 'native' }, f.scope());
  f.respond(8, 'applied', finalViewport);
  expect(await navigation).toBe(true);
  await settle();
  const obsolete = f.presentation.viewport(
    { type: 'pan', x: 0.2, y: 0, units: 'viewport' },
    f.scope(),
  );
  observe(f, lease, {
    type: 'preview',
    token: lease.preview.token,
    leaseRevision: lease.revision,
    viewport: previewViewport,
  });
  expect(await obsolete).toBe(false);
  expect(f.pending[9]!.cancelled).toBe(true);
  f.respond(9, 'stale');
  await settle();
  expect(f.previews.at(-1)).toBe(true);
  expect(f.viewports.at(-1)).toEqual(previewViewport);
  expect(f.statuses.at(-1)?.state).toBe('ready');
  expect(await f.presentation.viewport({ type: 'native' }, f.scope())).toBe(false);
  f.clock.advance(60_000);
  expect(f.calls).toHaveLength(10);
  f.presentation.update(previewGeometry, f.item, null);
  expect(f.calls[10]).toMatchObject({
    type: 'surface.preview.withdraw',
    leaseRevision: lease.revision,
  });
  f.respond(10);
  await settle();
  expect(f.previews.at(-1)).toBe(true);
  const selection = f.calls.find((call) => call.type === 'surface.select')!;
  observe(
    f,
    lease,
    { type: 'final', selectionRevision: selection.revision, viewport: finalViewport },
    'cancelled',
    2,
  );
  expect(f.previews.at(-1)).toBe(false);
  expect(f.viewports.at(-1)).toEqual(finalViewport);
  expect(f.errors).toEqual([]);
});

test('a stale final-camera reply before preview presentation preserves visibility and subsequent withdrawal', async () => {
  const f = fixture();
  await commit(f);
  const lease = await transfer(f);
  const navigation = f.presentation.viewport(
    { type: 'pan', x: 0.2, y: 0, units: 'viewport' },
    f.scope(),
  );
  f.respond(8, 'stale');
  await settle();
  expect(await navigation).toBe(false);
  expect(f.statuses.at(-1)?.state).toBe('unavailable');
  expect(f.calls).toHaveLength(9);
  expect(await f.presentation.viewport({ type: 'native' }, f.scope())).toBe(false);
  observe(
    f,
    { ...lease, revision: lease.revision + 1 },
    {
      type: 'preview',
      token: lease.preview.token,
      leaseRevision: lease.revision + 1,
      viewport: previewViewport,
    },
  );
  expect(f.previews).not.toContain(true);
  const shown = {
    type: 'preview' as const,
    token: lease.preview.token,
    leaseRevision: lease.revision,
    viewport: previewViewport,
  };
  f.emit({
    address: { ...f.address, attachmentId: 'foreign' },
    token: lease.preview.token,
    leaseRevision: lease.revision,
    sequence: 99,
    status: 'presented',
    displayed: shown,
    message: '',
  });
  observe(
    f,
    lease,
    { type: 'final', selectionRevision: 0, viewport: finalViewport },
    'cancelled',
    99,
  );
  expect(f.previews).not.toContain(true);
  observe(f, lease, shown);
  await settle();
  expect(f.previews.at(-1)).toBe(true);
  expect(f.viewports.at(-1)).toEqual(previewViewport);
  expect(f.statuses.at(-1)?.state).toBe('ready');
  expect(f.visible.at(-1)).toBe(true);
  expect(f.calls).toHaveLength(9);
  f.presentation.update(previewGeometry, f.item, null);
  expect(f.calls[9]).toMatchObject({
    type: 'surface.preview.withdraw',
    leaseRevision: lease.revision,
  });
  f.respond(9);
  await settle();
  const selection = f.calls.find((call) => call.type === 'surface.select')!;
  observe(
    f,
    lease,
    { type: 'final', selectionRevision: selection.revision, viewport: finalViewport },
    'cancelled',
    2,
  );
  await settle();
  expect(f.previews.at(-1)).toBe(false);
  expect(f.visible.at(-1)).toBe(true);
  const latest = f.presentation.viewport({ type: 'native' }, f.scope());
  expect(f.calls[10]).toMatchObject({
    type: 'surface.viewport.request',
    command: { type: 'native' },
  });
  f.respond(10, 'applied', finalViewport);
  expect(await latest).toBe(true);
  expect(f.calls.filter((call) => call.type === 'surface.prepare')).toHaveLength(1);
});

test.each(['failed', 'cancelled'] as const)(
  'a correlated %s observation restores final presentation after an earlier stale camera reply',
  async (status) => {
    const f = fixture();
    await commit(f);
    const lease = await transfer(f);
    const navigation = f.presentation.viewport({ type: 'native' }, f.scope());
    f.respond(8, 'stale');
    await settle();
    expect(await navigation).toBe(false);
    const selection = f.calls.find((call) => call.type === 'surface.select')!;
    observe(
      f,
      lease,
      { type: 'final', selectionRevision: selection.revision, viewport: finalViewport },
      status,
    );
    await settle();
    expect(f.visible.at(-1)).toBe(true);
    expect(f.previews.at(-1)).toBe(false);
    expect(f.viewports.at(-1)).toEqual(finalViewport);
    expect(f.calls).toHaveLength(9);
    const latest = f.presentation.viewport({ type: 'fit' }, f.scope());
    expect(f.calls[9]).toMatchObject({
      type: 'surface.viewport.request',
      command: { type: 'fit' },
    });
    f.respond(9, 'applied', finalViewport);
    expect(await latest).toBe(true);
  },
);

test('the original transfer deadline retains recovery correlation through accepted-then-withdrawn pixels', async () => {
  const f = fixture();
  await commit(f);
  const lease = await transfer(f);
  const navigation = f.presentation.viewport({ type: 'native' }, f.scope());
  f.respond(8, 'stale');
  await settle();
  expect(await navigation).toBe(false);
  f.clock.advance(30_000);
  expect(f.calls).toHaveLength(9);
  observe(f, lease, {
    type: 'preview',
    token: lease.preview.token,
    leaseRevision: lease.revision,
    viewport: previewViewport,
  });
  await settle();
  expect(f.visible.at(-1)).toBe(true);
  expect(f.previews.at(-1)).toBe(true);
  expect(f.calls[9]).toMatchObject({ type: 'surface.preview.withdraw' });
  f.respond(9);
  await settle();
  const selection = f.calls.find((call) => call.type === 'surface.select')!;
  observe(
    f,
    lease,
    { type: 'final', selectionRevision: selection.revision, viewport: finalViewport },
    'cancelled',
    2,
  );
  await settle();
  expect(f.visible.at(-1)).toBe(true);
  expect(f.previews.at(-1)).toBe(false);
  expect(f.presentation.diagnostics().activeDeadlines).toBe(0);
});

test('a transfer without an applied end cannot recover a stale camera through a claimed presentation', async () => {
  const f = fixture();
  await commit(f);
  const blob = new NativePreviewBlob();
  f.presentation.receive(f.preview(), blob);
  f.respond(5);
  await settle();
  const lease = begin(f);
  const navigation = f.presentation.viewport({ type: 'native' }, f.scope());
  f.respond(6, 'stale');
  await settle();
  expect(await navigation).toBe(false);
  expect(f.statuses.at(-1)?.state).toBe('unavailable');
  expect(f.calls).toHaveLength(7);
  observe(f, lease, {
    type: 'preview',
    token: lease.preview.token,
    leaseRevision: lease.revision,
    viewport: previewViewport,
  });
  await settle();
  expect(f.visible.at(-1)).toBe(false);
  expect(f.calls).toHaveLength(7);
  expect(f.calls.some((call) => call.type === 'surface.preview.end')).toBe(false);
  blob.release();
  await settle();
  expect(f.calls).toHaveLength(7);
});

test('a newer rejected geometry intent cannot resurrect the old stale-camera recovery', async () => {
  const f = fixture();
  await commit(f);
  const lease = await transfer(f);
  const navigation = f.presentation.viewport({ type: 'native' }, f.scope());
  f.respond(8, 'stale');
  await settle();
  expect(await navigation).toBe(false);
  const resized = {
    ...previewGeometry,
    viewport: { width: 700, height: 400 },
    clip: { ...previewGeometry.clip, width: 700 },
  };
  f.presentation.update(resized, f.item, f.preview());
  expect(f.calls[9]).toMatchObject({ type: 'surface.geometry', geometry: resized });
  f.respond(9, 'stale');
  await settle();
  expect(f.statuses.at(-1)?.state).toBe('unavailable');
  observe(f, lease, {
    type: 'preview',
    token: lease.preview.token,
    leaseRevision: lease.revision,
    viewport: previewViewport,
  });
  await settle();
  expect(f.visible.at(-1)).toBe(false);
  expect(f.calls).toHaveLength(10);
  f.presentation.update(resized, f.item, f.preview());
  expect(f.calls[10]).toMatchObject({ type: 'surface.geometry', geometry: resized });
  f.respond(10);
  await settle();
  expect(f.calls[11]).toMatchObject({
    type: 'surface.viewport.request',
    command: { type: 'read' },
  });
  f.respond(11, 'applied', previewViewport);
  await settle();
  expect(f.visible.at(-1)).toBe(true);
  expect(f.calls.filter((call) => call.type === 'surface.geometry')).toHaveLength(3);
});

test('retirement fences an admitted preview recovery and its already captured listener', async () => {
  const f = fixture();
  await commit(f);
  const lease = await transfer(f);
  const listener = [...f.listeners][0]!;
  const navigation = f.presentation.viewport({ type: 'native' }, f.scope());
  f.respond(8, 'stale');
  await settle();
  expect(await navigation).toBe(false);
  const retired = f.presentation.dispose();
  expect(f.calls[9]?.type).toBe('surface.release');
  listener({
    address: f.address,
    token: lease.preview.token,
    leaseRevision: lease.revision,
    sequence: 1,
    status: 'presented',
    displayed: {
      type: 'preview',
      token: lease.preview.token,
      leaseRevision: lease.revision,
      viewport: previewViewport,
    },
    message: '',
  });
  f.respond(9);
  await retired;
  expect(f.visible.at(-1)).toBe(false);
  expect(f.previews.at(-1)).toBe(false);
  expect(f.calls).toHaveLength(10);
});

test('keeps one active and only the latest pending frame, retaining the displayed preview when replacement fails', async () => {
  const f = fixture();
  await commit(f);
  const firstBlob = new NativePreviewBlob();
  f.presentation.receive(f.preview(), firstBlob);
  f.respond(5);
  await settle();
  const middle = new NativePreviewBlob();
  f.presentation.update(previewGeometry, f.item, f.preview(2));
  f.presentation.receive(f.preview(2), middle);
  const latest = new NativePreviewBlob();
  f.presentation.update(previewGeometry, f.item, f.preview(3));
  f.presentation.receive(f.preview(3), latest);
  expect(f.presentation.diagnostics().queuedInputs).toBe(1);
  expect(f.presentation.diagnostics().previewDrops).toBe(1);
  firstBlob.release();
  await settle();
  f.respond(6);
  await settle();
  f.respond(7);
  await settle();
  const first = begin(f);
  observe(f, first, {
    type: 'preview',
    token: first.preview.token,
    leaseRevision: first.revision,
    viewport: previewViewport,
  });
  await settle();
  expect(f.calls[8]).toMatchObject({
    type: 'surface.preview.begin',
    preview: { token: 'preview:3' },
  });
  expect(middle.reads).toHaveLength(0);
  f.respond(8);
  await settle();
  const replacement = begin(f, 'preview:3');
  observe(
    f,
    replacement,
    {
      type: 'preview',
      token: first.preview.token,
      leaseRevision: first.revision,
      viewport: previewViewport,
    },
    'failed',
    2,
  );
  expect(f.previews.at(-1)).toBe(true);
  expect(f.viewports.at(-1)).toEqual(previewViewport);
  expect(f.presentation.diagnostics()).toMatchObject({
    estimatedBytes: 0,
    activeDeadlines: 0,
    queuedInputs: 0,
  });
  expect(f.errors).toHaveLength(1);
});

test('commits visible empty native geometry so a first-ever preview needs no selected final', async () => {
  const f = fixture();
  await commit(f, null);
  expect(f.calls.map((call) => call.type)).toEqual([
    'surface.prepare',
    'surface.geometry',
    'surface.commit',
  ]);
  expect(f.calls[1]).toMatchObject({ geometry: { visible: true } });
  const blob = new NativePreviewBlob();
  f.presentation.receive(f.preview(), blob);
  f.respond(3);
  await settle();
  blob.release();
  await settle();
  f.respond(4);
  await settle();
  f.respond(5);
  await settle();
  expect(f.previews).not.toContain(true);
  const lease = begin(f);
  observe(f, lease, {
    type: 'preview',
    token: lease.preview.token,
    leaseRevision: lease.revision,
    viewport: previewViewport,
  });
  expect(f.previews.at(-1)).toBe(true);
  expect(f.statuses.at(-1)?.state).toBe('ready');
});

test('does not renew the begin deadline and withdraws without waiting for a late Blob read', async () => {
  const f = fixture();
  await commit(f);
  const blob = new NativePreviewBlob(8195);
  f.presentation.receive(f.preview(), blob);
  f.respond(5);
  await settle();
  f.clock.advance(29_000);
  blob.release();
  await settle();
  f.respond(6);
  await settle();
  expect(blob.reads).toHaveLength(2);
  f.clock.advance(1000);
  await settle();
  expect(f.calls[7]?.type).toBe('surface.preview.withdraw');
  expect(f.presentation.diagnostics()).toMatchObject({ estimatedBytes: 0, activeDeadlines: 0 });
  f.respond(7);
  await settle();
  blob.release(1);
  await settle();
  expect(f.calls.filter((call) => call.type === 'surface.preview.chunk')).toHaveLength(1);
  expect(f.previews).not.toContain(true);
});

test('rejects stale identities and late callbacks after actual surface retirement', async () => {
  const f = fixture();
  await commit(f);
  const lease = await transfer(f);
  const displayed = {
    type: 'preview' as const,
    token: lease.preview.token,
    leaseRevision: lease.revision,
    viewport: previewViewport,
  };
  f.emit({
    address: { ...f.address, generation: f.address.generation + 1 },
    sequence: 99,
    token: lease.preview.token,
    leaseRevision: lease.revision,
    status: 'presented',
    displayed,
    message: '',
  });
  observe(f, { ...lease, revision: lease.revision + 100 }, displayed, 'presented', 90);
  expect(f.previews).not.toContain(true);
  observe(f, lease, displayed, 'presented', 1);
  expect(f.previews.at(-1)).toBe(true);
  const listener = [...f.listeners][0]!;
  const retired = f.presentation.dispose();
  expect(f.stop).toHaveBeenCalledTimes(1);
  expect(f.clock.pending).toBe(0);
  const count = f.previews.length;
  listener({
    address: f.address,
    sequence: 2,
    token: lease.preview.token,
    leaseRevision: lease.revision,
    status: 'presented',
    displayed,
    message: '',
  });
  expect(f.previews).toHaveLength(count);
  f.respond(f.calls.length - 1);
  await retired;
});

test('keeps unsupported hosts allocation-free and rejects foreign, oversized or non-JPEG bytes', async () => {
  const unsupported = fixture(false);
  await commit(unsupported);
  const refused = new NativePreviewBlob();
  unsupported.presentation.receive(unsupported.preview(), refused);
  expect(unsupported.presentation.wants()).toBe(false);
  expect(refused.reads).toHaveLength(0);
  expect(unsupported.calls).toHaveLength(5);
  const f = fixture();
  await commit(f);
  const foreign = { ...f.preview(), sourceId: 'source:other' as const };
  f.presentation.receive(foreign, new NativePreviewBlob());
  f.presentation.receive(f.preview(), new Blob([new Uint8Array(3)], { type: 'image/png' }));
  const oversized = new NativePreviewBlob();
  Object.defineProperty(oversized, 'size', { value: 64 * 1024 * 1024 + 1 });
  f.presentation.receive(f.preview(), oversized);
  expect(f.calls).toHaveLength(5);
  expect(f.presentation.diagnostics().estimatedBytes).toBe(0);
});

test.each([true, false])(
  'retains withdrawal correlation until actual restoration with retained final %s',
  async (hasFinal) => {
    const f = fixture();
    const item = hasFinal ? f.item : null;
    await commit(f, item);
    const start = f.calls.length;
    const firstBlob = new NativePreviewBlob();
    f.presentation.receive(f.preview(), firstBlob);
    f.respond(start);
    await settle();
    firstBlob.release();
    await settle();
    f.respond(start + 1);
    await settle();
    f.respond(start + 2);
    await settle();
    const first = begin(f);
    observe(f, first, {
      type: 'preview',
      token: first.preview.token,
      leaseRevision: first.revision,
      viewport: previewViewport,
    });
    await settle();
    f.presentation.update(previewGeometry, item, f.preview(2));
    const secondBlob = new NativePreviewBlob();
    f.presentation.receive(f.preview(2), secondBlob);
    await settle();
    const secondIndex = f.calls.length - 1;
    f.respond(secondIndex);
    await settle();
    const second = begin(f, 'preview:2');
    f.presentation.update(previewGeometry, item, null);
    expect(f.calls.at(-1)).toMatchObject({ type: 'surface.preview.withdraw', token: 'preview:2' });
    f.respond(f.calls.length - 1);
    await settle();
    expect(f.previews.at(-1)).toBe(true);
    const selection = f.calls.find((call) => call.type === 'surface.select');
    observe(
      f,
      second,
      selection
        ? { type: 'final', selectionRevision: selection.revision, viewport: finalViewport }
        : { type: 'empty' },
      'cancelled',
      2,
    );
    expect(f.previews.at(-1)).toBe(false);
    expect(f.statuses.at(-1)?.state).toBe(hasFinal ? 'ready' : 'empty');
    expect(f.presentation.diagnostics()).toMatchObject({ estimatedBytes: 0, activeDeadlines: 0 });
  },
);

test('reports accepted pixels racing withdrawal and retains their correlation until actual restoration', async () => {
  const f = fixture();
  await commit(f);
  const lease = await transfer(f);
  f.presentation.update(previewGeometry, f.item, null);
  expect(f.calls.at(-1)?.type).toBe('surface.preview.withdraw');
  observe(f, lease, {
    type: 'preview',
    token: lease.preview.token,
    leaseRevision: lease.revision,
    viewport: previewViewport,
  });
  expect(f.previews.at(-1)).toBe(true);
  f.respond(f.calls.length - 1);
  await settle();
  const selection = f.calls.find((call) => call.type === 'surface.select')!;
  observe(
    f,
    lease,
    { type: 'final', selectionRevision: selection.revision, viewport: finalViewport },
    'cancelled',
    2,
  );
  expect(f.previews.at(-1)).toBe(false);
  expect(f.viewports.at(-1)).toEqual(finalViewport);
});

test('a new final selection fences incoming preview bytes without restoring the previous final first', async () => {
  const f = fixture();
  await commit(f);
  const blob = new NativePreviewBlob();
  f.presentation.receive(f.preview(), blob);
  f.respond(5);
  await settle();
  const old = begin(f);
  f.presentation.update(previewGeometry, f.second, null);
  expect(f.calls[6]).toMatchObject({ type: 'surface.select', itemId: f.second.id });
  expect(f.calls.some((call) => call.type === 'surface.preview.withdraw')).toBe(false);
  blob.release();
  await settle();
  expect(f.calls.filter((call) => call.type === 'surface.preview.chunk')).toHaveLength(0);
  f.respond(6);
  await settle();
  f.respond(7, 'applied', finalViewport);
  await settle();
  observe(f, old, {
    type: 'preview',
    token: old.preview.token,
    leaseRevision: old.revision,
    viewport: previewViewport,
  });
  expect(f.previews).not.toContain(true);
  expect(f.presentation.diagnostics()).toMatchObject({ estimatedBytes: 0, activeDeadlines: 0 });
});

test.each(['error', 'unavailable'] as const)(
  'a rejected final selection (%s) retains truthful preview and restoration correlation',
  async (status) => {
    const f = fixture();
    await commit(f);
    const lease = await transfer(f);
    const selection = f.calls.find((call) => call.type === 'surface.select')!;
    const shown = {
      type: 'preview' as const,
      token: lease.preview.token,
      leaseRevision: lease.revision,
      viewport: previewViewport,
    };
    observe(f, lease, shown);
    await settle();
    f.presentation.update(previewGeometry, f.second, null);
    await settle();
    const pendingSelect = f.calls.length - 1;
    expect(f.calls[pendingSelect]).toMatchObject({ type: 'surface.select', itemId: f.second.id });
    observe(f, lease, shown, 'presented', 2);
    expect(f.previews.at(-1)).toBe(true);
    expect(f.viewports.at(-1)).toEqual(previewViewport);
    f.respond(pendingSelect, status);
    await settle();
    expect(f.previews.at(-1)).toBe(true);
    expect(f.viewports.at(-1)).toEqual(previewViewport);
    expect(f.presentation.diagnostics()).toMatchObject({ estimatedBytes: 0, activeDeadlines: 0 });
    observe(
      f,
      lease,
      { type: 'final', selectionRevision: selection.revision, viewport: finalViewport },
      'cancelled',
      3,
    );
    expect(f.previews.at(-1)).toBe(false);
    expect(f.viewports.at(-1)).toEqual(finalViewport);
  },
);

test.each(['stale', 'unavailable', 'error'] as const)(
  'a refused withdraw (%s) releases the parked transfer so the next preview can begin',
  async (status) => {
    const f = fixture();
    await commit(f);
    const blob = new NativePreviewBlob();
    f.presentation.receive(f.preview(), blob);
    f.respond(5);
    await settle();
    blob.release();
    await settle();
    expect(f.calls[6]?.type).toBe('surface.preview.chunk');
    f.respond(6, 'error');
    await settle();
    expect(f.calls[7]).toMatchObject({ type: 'surface.preview.withdraw', token: 'preview:1' });
    f.respond(7, status);
    await settle();
    expect(f.presentation.diagnostics()).toMatchObject({ estimatedBytes: 0, activeDeadlines: 0 });
    f.presentation.update(previewGeometry, f.item, f.preview(2));
    f.presentation.receive(f.preview(2), new NativePreviewBlob());
    await settle();
    expect(f.calls.at(-1)).toMatchObject({
      type: 'surface.preview.begin',
      preview: { token: 'preview:2' },
    });
  },
);

test('workflow retirement fences a pending byte extraction and its late native observation', async () => {
  const f = fixture();
  await commit(f);
  const blob = new NativePreviewBlob();
  f.presentation.receive(f.preview(), blob);
  f.respond(5);
  await settle();
  const old = begin(f);
  f.deactivate();
  f.presentation.update(previewGeometry, f.item, { ...f.preview(), generation: 1 });
  expect(f.calls[6]?.type).toBe('surface.release');
  expect(f.stop).toHaveBeenCalledTimes(1);
  blob.release();
  await settle();
  observe(f, old, {
    type: 'preview',
    token: old.preview.token,
    leaseRevision: old.revision,
    viewport: previewViewport,
  });
  expect(f.calls).toHaveLength(7);
  expect(f.previews.at(-1)).toBe(false);
  expect(f.clock.pending).toBe(0);
  f.respond(6);
  await settle();
});

test.each(['unsupported', 'unavailable'] as const)(
  'an observation outcome of %s cannot allocate an empty preview surface or receive bytes',
  async (status) => {
    const f = fixture();
    f.presentation.update(previewGeometry, null, f.preview());
    f.ready(status);
    await settle();
    const blob = new NativePreviewBlob();
    f.presentation.receive(f.preview(), blob);
    expect(f.presentation.wants()).toBe(false);
    expect(f.calls).toHaveLength(0);
    expect(blob.reads).toHaveLength(0);
  },
);

test('retirement rejects an observation acknowledgement that arrives after its subscriber stopped', async () => {
  const f = fixture();
  f.presentation.update(previewGeometry, null, f.preview());
  await f.presentation.dispose();
  f.ready('observing');
  await settle();
  f.presentation.receive(f.preview(), new NativePreviewBlob());
  expect(f.calls).toHaveLength(0);
  expect(f.stop).toHaveBeenCalledTimes(1);
});
