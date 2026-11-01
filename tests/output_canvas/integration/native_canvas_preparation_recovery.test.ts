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
/** Prove a native preparation that times out under host load recovers instead of staying blank. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { CanvasPrimarySurface } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasPrimarySurface.js';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import type { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import {
  nativePreviewHost,
  previewGeometry,
  finalViewport,
  settleNativePreview as settle,
} from '../support/NativeCanvasPreviewFixture.js';

const disposals: (() => void)[] = [];
afterEach(() => {
  for (const dispose of disposals.splice(0)) dispose();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

/** Compose the real primary surface, attachments and presentation over a slow native host. */
function fixture() {
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 500,
    bottom: 400,
    width: 500,
    height: 400,
    toJSON: () => ({}),
  });
  // jsdom cannot measure visibility; keep the observed allocation drawable.
  jest.spyOn(CanvasSurfaceGeometryObserver.prototype, 'read').mockReturnValue(previewGeometry);
  const catalog = canvasCatalogFixture();
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(view, catalog);
  const coordinator = {
    active: () => view,
    catalog: () => catalog,
    subscribe: () => () => undefined,
  } as unknown as CanvasSessionCoordinator;
  const element = document.createElement('div');
  document.body.append(element);
  const clock = new WorkflowTestClock();
  const host = nativePreviewHost(false);
  const errors: unknown[] = [];
  const attachments: NativeCanvasAttachment[] = [];
  const primary = new CanvasPrimarySurface({
    document,
    window: windowId('native-recovery'),
    views,
    coordinator,
    clock,
    element: () => element,
    renderer: () => 'native',
    report: (error) => errors.push(error),
    mount: (root, address, _catalog, accepts, acceptsPreparation, recover) => {
      const attachment = new NativeCanvasAttachment(root, {
        address,
        clock,
        views,
        selection,
        frames: new CanvasFrameFixture(),
        inspections: new CanvasInspectionStore(),
        catalog: () => catalog,
        accepts,
        acceptsPreparation,
        nativeSurface: () => host.host,
        connected: () => true,
        report: (error) => errors.push(error),
        recover,
      });
      attachments.push(attachment);
      return attachment;
    },
  });
  disposals.push(() => primary.dispose());
  /** Attachments whose host preparation never answers, as under the reported host load. */
  const hung = new Set<string>();
  return {
    ...host,
    primary,
    attachments,
    clock,
    errors,
    hung,
    hangAll: true,
    /** Acknowledge every ticket except preparation of a hung attachment. */
    async drain(this: { hangAll: boolean }): Promise<void> {
      for (let guard = 0; guard < 32; guard++) {
        await settle();
        const index = host.pending.findIndex(
          (record, position) =>
            !record.settled &&
            !(
              host.calls[position]?.type === 'surface.prepare' &&
              (this.hangAll || hung.has(host.calls[position]!.address.attachmentId))
            ),
        );
        if (index < 0) return;
        const call = host.calls[index]!;
        host.respond(
          index,
          call.type === 'surface.prepare' ? 'ready' : 'applied',
          call.type === 'surface.viewport.request' ? finalViewport : undefined,
        );
      }
      throw new Error('Native operations did not converge.');
    },
    prepares: () => host.calls.filter(({ type }) => type === 'surface.prepare'),
    select: (item: (typeof catalog.entries)[number]['items'][number]) =>
      selection.apply(view, catalog, {
        type: 'select-source',
        selection: { sourceId: item.provenance.sourceId, batchIndex: item.batchIndex },
      }),
    other: () => {
      const selected = views.read(view).selection;
      return catalog.entries.find((entry) => entry.source.id !== selected?.sourceId)!.items[0]!;
    },
  };
}

test('a preparation timeout shows loading, retries afresh and then presents the same final', async () => {
  const f = fixture();
  f.primary.setup();
  await f.drain();
  expect(f.prepares()).toHaveLength(1);
  f.clock.advance(5000);
  await f.drain();
  // The timed-out candidate is committed so its loading state replaces a blank pane.
  const [stalled] = f.attachments;
  expect(f.primary.diagnostics().address).toEqual(stalled!.port.read().address);
  expect(stalled!.port.read().resource.state).toBe('loading');
  expect(f.errors.some((error) => String(error).includes('timed out'))).toBe(true);
  // The host recovers; the backoff mounts a fresh attachment for the unchanged selection.
  f.hangAll = false;
  f.hung.add(f.prepares()[0]!.address.attachmentId);
  f.clock.advance(1000);
  await f.drain();
  expect(f.prepares()).toHaveLength(2);
  expect(f.prepares()[1]!.address.attachmentId).not.toBe(f.prepares()[0]!.address.attachmentId);
  const fresh = f.attachments[1]!;
  expect(f.primary.diagnostics().address).toEqual(fresh.port.read().address);
  expect(fresh.port.read().resource.state).toBe('ready');
  expect(fresh.viewport.classList.contains('sc-output-native')).toBe(true);
  expect(
    f.calls.some(
      (call) =>
        call.type === 'surface.release' &&
        call.address.attachmentId === f.prepares()[0]!.address.attachmentId,
    ),
  ).toBe(true);
  expect(f.clock.pending).toBe(0);
});

test('exhausted retries show the preparation error and a later selection attaches afresh', async () => {
  const f = fixture();
  f.primary.setup();
  await f.drain();
  for (const delay of [5000, 1000, 5000, 2000, 5000, 4000, 5000]) {
    f.clock.advance(delay);
    await f.drain();
  }
  expect(f.prepares()).toHaveLength(4);
  const current = f.attachments[0]!;
  expect(f.primary.diagnostics().address).toEqual(current.port.read().address);
  expect(current.port.read().resource).toMatchObject({
    state: 'error',
    message: 'The native canvas could not be prepared.',
  });
  expect(f.clock.pending).toBe(0);
  // The host recovers; a later selection attaches afresh with a new retry budget.
  f.hangAll = false;
  for (const prepare of f.prepares()) f.hung.add(prepare.address.attachmentId);
  const next = f.other();
  f.select(next);
  await f.drain();
  expect(f.prepares()).toHaveLength(5);
  const fresh = f.attachments.at(-1)!;
  expect(f.primary.diagnostics().address).toEqual(fresh.port.read().address);
  expect(f.calls.filter(({ type }) => type === 'surface.select').at(-1)).toMatchObject({
    itemId: next.id,
  });
  expect(fresh.port.read().resource.state).toBe('ready');
  expect(fresh.viewport.classList.contains('sc-output-native')).toBe(true);
});
