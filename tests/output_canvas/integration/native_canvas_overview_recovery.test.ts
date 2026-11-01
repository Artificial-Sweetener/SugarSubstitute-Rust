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
/** Prove a startup overview whose native acquisition times out re-attaches and is shown. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { CanvasPrimarySurface } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasPrimarySurface.js';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { canvasOutputCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import type { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import {
  promptId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type { WorkflowResultSession } from '../../../frontend/comfyui/ui/execution/domain/WorkflowResultSession.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasSources } from '../support/CanvasCatalogFixture.js';
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

/** One source with a batch of four images, which opens as an overview grid. */
function batchCatalog() {
  const [source] = canvasSources();
  if (!source) throw new Error('Missing source.');
  const context = { sessionId: workflowSessionId('recovery'), generation: 0, title: '', path: '' };
  const provenance = {
    ...context,
    sourceId: source.id,
    executionPath: source.executionPath,
    runOrder: 1,
    sourceOrder: 0,
    revision: 1,
    kind: 'execution' as const,
    promptId: promptId('1'),
  };
  const items: WorkflowOutputItem[] = [0, 1, 2, 3].map((batchIndex) => ({
    id: `result:recovery-${batchIndex}`,
    provenance,
    batchIndex,
    file: { filename: `ComfyUI_${batchIndex}.png`, subfolder: '', type: 'output' },
    mediaKind: 'image',
    dimensions: { width: 1024, height: 1024 },
  }));
  const result = {
    context,
    revision: 1,
    outputs: [{ source, provenance, items, batchCount: 4, text: [] }],
    runOrder: 1,
    presentedRunOrder: 1,
    runStatus: 'complete',
    preview: null,
    recovery: { state: 'idle', message: '' },
  } as unknown as WorkflowResultSession;
  return canvasOutputCatalog(result, [source]);
}

/**
 * Commit a first attachment under an earlier renderer identity, then let discovery replace the
 * renderer so a candidate for the startup overview is staged while that attachment remains.
 */
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
  const catalog = batchCatalog();
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
  const native = nativePreviewHost(false);
  const host = { ...native.host, capabilities: { ...native.host.capabilities, overview: true } };
  const errors: unknown[] = [];
  const attachments: NativeCanvasAttachment[] = [];
  let renderer = 'native:discovering';
  const primary = new CanvasPrimarySurface({
    document,
    window: windowId('native-overview-recovery'),
    views,
    coordinator,
    clock,
    element: () => element,
    renderer: () => renderer,
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
        nativeSurface: () => host,
        connected: () => true,
        report: (error) => errors.push(error),
        recover,
      });
      attachments.push(attachment);
      return attachment;
    },
  });
  disposals.push(() => primary.dispose());
  /** Attachments whose overview stays in host acquisition, as under the reported host load. */
  const acquiring = new Set<string>();
  return {
    ...native,
    primary,
    attachments,
    clock,
    errors,
    acquiring,
    discover: () => {
      renderer = 'native:overview';
      primary.reconcile();
    },
    /** Acknowledge every ticket except an overview still acquiring on the host. */
    async drain(): Promise<void> {
      for (let guard = 0; guard < 32; guard++) {
        await settle();
        const index = native.pending.findIndex(
          (record, position) =>
            !record.settled &&
            !(
              native.calls[position]?.type === 'surface.overview' &&
              acquiring.has(native.calls[position]!.address.attachmentId)
            ),
        );
        if (index < 0) return;
        const call = native.calls[index]!;
        native.respond(
          index,
          call.type === 'surface.prepare' ? 'ready' : 'applied',
          call.type === 'surface.viewport.request' ? finalViewport : undefined,
        );
      }
      throw new Error('Native operations did not converge.');
    },
    prepares: () => native.calls.filter(({ type }) => type === 'surface.prepare'),
  };
}

test('a startup overview that times out in host acquisition re-attaches and is shown', async () => {
  const f = fixture();
  f.primary.setup();
  await f.drain();
  expect(f.attachments[0]!.port.read().view.mode).toBe('overview');
  // Discovery replaces the renderer; the staged overview candidate stalls in acquisition.
  f.discover();
  await settle();
  const candidate = f.prepares().at(-1)!.address.attachmentId;
  f.acquiring.add(candidate);
  await f.drain();
  expect(f.calls.at(-1)).toMatchObject({ type: 'surface.overview' });
  f.clock.advance(5000);
  await f.drain();
  // The timed-out candidate replaces the outdated attachment and shows its loading state.
  const stalled = f.attachments[1]!;
  expect(f.primary.diagnostics().address).toEqual(stalled.port.read().address);
  expect(stalled.port.read().resource.state).not.toBe('unavailable');
  // The backoff re-attaches afresh, and the host now answers the overview.
  f.clock.advance(1000);
  await f.drain();
  const fresh = f.attachments.at(-1)!;
  expect(fresh).not.toBe(stalled);
  expect(f.prepares().at(-1)!.address.attachmentId).toBe(fresh.port.read().address.attachmentId);
  expect(f.primary.diagnostics().address).toEqual(fresh.port.read().address);
  const shown = f.calls.filter(
    (call) =>
      call.type === 'surface.overview' &&
      call.address.attachmentId === fresh.port.read().address.attachmentId,
  );
  expect(shown).toHaveLength(1);
  expect(fresh.port.read().view.mode).toBe('overview');
  expect(fresh.viewport.classList.contains('sc-output-native')).toBe(true);
  expect(
    f.calls.some(
      (call) => call.type === 'surface.release' && call.address.attachmentId === candidate,
    ),
  ).toBe(true);
  expect(f.clock.pending).toBe(0);
});

test('an overview that never leaves acquisition ends in the preparation error', async () => {
  const f = fixture();
  f.primary.setup();
  await f.drain();
  f.discover();
  for (const delay of [0, 5000, 1000, 5000, 2000, 5000, 4000, 5000]) {
    f.clock.advance(delay);
    await settle();
    for (const prepare of f.prepares()) f.acquiring.add(prepare.address.attachmentId);
    await f.drain();
  }
  const current = f.attachments.find(
    (attachment) =>
      attachment.port.read().address.attachmentId === f.primary.diagnostics().address?.attachmentId,
  )!;
  expect(current).not.toBe(f.attachments[0]);
  expect(current.port.read().resource).toMatchObject({
    state: 'error',
    message: 'The native canvas could not be prepared.',
  });
  expect(f.clock.pending).toBe(0);
});
