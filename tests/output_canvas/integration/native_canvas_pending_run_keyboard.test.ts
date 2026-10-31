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
/** Keep a native final navigable by keyboard while a newer Run waits for its first preview. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { NativeCanvasPresentation } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasPresentation.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { canvasOutputCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { canvasRevealTarget } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasReveal.js';
import type { WorkflowPreview } from '../../../frontend/comfyui/ui/execution/domain/WorkflowPreview.js';
import type { WorkflowResultSession } from '../../../frontend/comfyui/ui/execution/domain/WorkflowResultSession.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import type { CanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasCatalogFixture, canvasSources } from '../support/CanvasCatalogFixture.js';
import {
  nativePreviewHost,
  previewGeometry,
  finalViewport,
  settleNativePreview as settle,
} from '../support/NativeCanvasPreviewFixture.js';

const retirements: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const retire of retirements.splice(0)) await retire();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

/** How the result store describes the newer Run while ComfyUI holds it before any JPEG. */
type PendingRun = 'gated' | 'preview-metadata';

interface Scenario {
  readonly pending: PendingRun;
  readonly reveal: boolean;
  readonly staged: boolean;
  /** Deliver the key while the refocus geometry acknowledgement is still pending. */
  readonly early: boolean;
}

/** Mount real view, selection, port, keyboard and presentation owners over a fake native host. */
function fixture({ pending, staged }: Scenario) {
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
  const viewportCalls = jest.spyOn(NativeCanvasPresentation.prototype, 'viewport');
  const original = canvasCatalogFixture(1, 0, canvasSources().slice(0, 1));
  const source = original.sources[0]!;
  const preview: WorkflowPreview = {
    sessionId: original.result.context.sessionId,
    generation: 0,
    promptId: 'prompt:pending',
    sourceId: source.id,
    runOrder: 2,
    sequence: 1,
    token: 'preview:pending',
    dimensions: null,
  };
  let catalog: CanvasCatalogSnapshot = original;
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  let latest = 1;
  const selection = new CanvasSelectionController(views, { latestOrder: () => latest });
  selection.reconcile(view, catalog);
  const address = views.prepareAttachment(view, windowId('native-pending-run'));
  if (!staged) views.commitAttachment(address);
  const root = document.createElement('section');
  document.body.append(root);
  const host = nativePreviewHost();
  const errors: unknown[] = [];
  const surface = new NativeCanvasAttachment(root, {
    address,
    clock: new WorkflowTestClock(),
    views,
    selection,
    frames: new CanvasFrameFixture(),
    inspections: new CanvasInspectionStore(),
    catalog: () => catalog,
    accepts: () => views.accepts(address),
    acceptsPreparation: () => staged && views.acceptsPreparation(address),
    nativeSurface: () => host.host,
    connected: () => true,
    open: () => {},
    report: (error) => errors.push(error),
  });
  const geometry = (value: CanvasSurfaceGeometry) => surface.port.updateSurface(address, value);
  geometry({ ...previewGeometry, focused: true });
  retirements.push(async () => {
    const retired = surface.dispose();
    for (const [index, record] of host.pending.entries())
      if (!record.settled)
        host.respond(
          index,
          host.calls[index]?.type === 'surface.release' ? 'applied' : 'cancelled',
        );
    await retired;
  });
  const publish = (result: WorkflowResultSession) => {
    catalog = canvasOutputCatalog(result, original.sources);
    selection.reconcile(view, catalog);
    surface.refresh();
  };
  return {
    ...host,
    surface,
    errors,
    viewportCalls,
    geometry,
    views,
    view,
    selection,
    commitStaged: () => {
      views.commitAttachment(address);
      surface.refresh();
      geometry({ ...previewGeometry, focused: false });
    },
    reveal: async () => {
      const item = original.entries[0]!.items[0]!;
      expect(await surface.reveal(canvasRevealTarget(item))).toBe(true);
    },
    /** Queue a newer Run held at ComfyUI's preview gate: no preview bytes have arrived. */
    queue: () => {
      latest = 2;
      const queued: WorkflowResultSession = {
        ...original.result,
        runOrder: 2,
        runStatus: 'queued',
        preview: null,
      };
      publish(queued);
      publish({ ...queued, runStatus: 'running' });
      if (pending === 'preview-metadata')
        publish({ ...queued, runStatus: 'running', presentedRunOrder: 2, preview });
    },
    /** Acknowledge every outstanding native ticket as the real host did. */
    async drain(): Promise<void> {
      for (let guard = 0; guard < 16; guard++) {
        await settle();
        const index = host.pending.findIndex((record) => !record.settled);
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
    arrowRight: () =>
      surface.viewport.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true, cancelable: true }),
      ),
    viewportRequests: () => host.calls.filter((call) => call.type === 'surface.viewport.request'),
  };
}

const scenarios: Scenario[] = [];
for (const pending of ['gated', 'preview-metadata'] as const)
  for (const reveal of [false, true])
    for (const staged of [false, true])
      for (const early of [false, true]) scenarios.push({ pending, reveal, staged, early });

for (const scenario of scenarios) {
  test(`keyboard pans a native final while a newer Run awaits its first preview ${JSON.stringify(
    scenario,
  )}`, async () => {
    const f = fixture(scenario);
    f.ready('observing');
    await f.drain();
    if (scenario.staged) {
      f.commitStaged();
      await f.drain();
    }
    await f.surface.ready;
    if (scenario.reveal) {
      await f.reveal();
      await f.drain();
    }
    for (const command of [{ type: 'native' }, { type: 'zoom', factor: 1.25 }] as const) {
      const outcome = f.surface.port.dispatch(
        f.surface.port.createRequest(command, f.surface.port.read().revision),
      );
      await f.drain();
      expect((await outcome).status).toBe('applied');
    }
    const dispatch = jest.spyOn(f.surface.port, 'dispatch');
    f.arrowRight();
    await f.drain();
    expect(dispatch).toHaveBeenCalledTimes(1);
    const keyed = dispatch.mock.results[0]!.value as ReturnType<typeof f.surface.port.dispatch>;
    expect((await keyed).status).toBe('applied');

    // Generate takes focus, then the queued Run is published, then the viewer is refocused.
    f.geometry({ ...previewGeometry, focused: false });
    f.queue();
    await f.drain();
    f.geometry({ ...previewGeometry, focused: true });
    if (!scenario.early) await f.drain();
    expect(f.surface.port.read().preview).toBe(false);

    const requestsBefore = f.viewportRequests().length;
    const viewportBefore = f.viewportCalls.mock.calls.length;
    dispatch.mockClear();
    f.arrowRight();
    // Attribute any refusal to the exact owner in the keyboard -> action -> presentation chain.
    expect({ keyReachedPort: dispatch.mock.calls.length }).toEqual({ keyReachedPort: 1 });
    expect({
      presentationViewportCalls: f.viewportCalls.mock.calls.length - viewportBefore,
    }).toEqual({ presentationViewportCalls: 1 });
    const admitted = f.viewportCalls.mock.results.at(-1)!.value;
    await f.drain();
    expect({ presentationAdmitted: await admitted }).toEqual({ presentationAdmitted: true });
    expect(f.viewportRequests().slice(requestsBefore)).toEqual([
      expect.objectContaining({ command: expect.objectContaining({ type: 'pan' }) }),
    ]);
    expect(f.errors).toEqual([]);
  });
}
