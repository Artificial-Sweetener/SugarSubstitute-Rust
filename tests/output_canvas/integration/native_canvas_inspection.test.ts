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
/** Record and restore native cameras through real view, selection and inspection owners. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import type { CanvasCommand } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfaceInspection,
  CanvasSurfaceViewportState,
} from '../../../integrations/contracts/CanvasSurface.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import {
  nativePreviewHost,
  previewGeometry,
  settleNativePreview as settle,
} from '../support/NativeCanvasPreviewFixture.js';

const retirements: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const retire of retirements.splice(0)) await retire();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

const inspecting: Partial<CanvasSurfaceCapabilities> = {
  viewportCommands: ['fit', 'native', 'zoom', 'pan', 'inspect'],
};
const fit = (): CanvasSurfaceViewportState => ({
  mode: 'fit',
  physicalScale: 0.13,
  dimensions: { width: 3840, height: 2160 },
  center: { x: 0.5, y: 0.5 },
});
/** A zoomed host camera over the fixture's 3840x2160 outputs. */
function zoomed(x: number, y: number, physicalScale = 2): CanvasSurfaceViewportState {
  return {
    mode: 'custom',
    physicalScale,
    dimensions: { width: 3840, height: 2160 },
    center: { x, y },
  };
}
/** The wire inspection that a zoomed camera restores on the same-sized image. */
function restored(state: CanvasSurfaceViewportState): CanvasSurfaceInspection {
  return {
    center: state.center ?? { x: 0.5, y: 0.5 },
    span: {
      x: 1 / (state.dimensions.width * state.physicalScale),
      y: 1 / (state.dimensions.height * state.physicalScale),
    },
  };
}

/** Share one view registry and inspection owner across workflows, like the canvas host. */
function workspace() {
  jest.spyOn(CanvasSurfaceGeometryObserver.prototype, 'read').mockReturnValue(previewGeometry);
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  const inspections = new CanvasInspectionStore();
  /** Open one workflow's view; its attachments come and go as tabs switch. */
  function workflow(key: string) {
    let catalog: CanvasCatalogSnapshot = canvasCatalogFixture(1, 0, undefined, key);
    const view = views.create(catalog.sessionId).viewId;
    selection.reconcile(view, catalog);
    return {
      view,
      get catalog() {
        return catalog;
      },
      /** Present a later Run's outputs in the same workflow generation. */
      run(order: number): void {
        catalog = canvasCatalogFixture(order, 0, undefined, key);
        selection.reconcile(view, catalog);
      },
      /** Mount a native attachment over a fake host answering camera reads with `camera`. */
      mount(capabilities: Partial<CanvasSurfaceCapabilities> = inspecting) {
        const address = views.prepareAttachment(view, windowId(`window-${key}-${++sequence}`));
        views.commitAttachment(address);
        const root = document.createElement('section');
        document.body.append(root);
        const native = nativePreviewHost(false);
        const host = {
          ...native.host,
          capabilities: { ...native.host.capabilities, ...capabilities },
        };
        const errors: unknown[] = [];
        let camera = fit();
        const surface = new NativeCanvasAttachment(root, {
          address,
          clock: new WorkflowTestClock(),
          views,
          selection,
          frames: new CanvasFrameFixture(),
          inspections,
          catalog: () => catalog,
          accepts: () => views.accepts(address),
          acceptsPreparation: () => false,
          nativeSurface: () => host,
          connected: () => true,
          open: () => {},
          report: (error) => errors.push(error),
        });
        surface.port.updateSurface(address, previewGeometry);
        let retired = false;
        const retire = async (): Promise<void> => {
          if (retired) return;
          retired = true;
          const released = surface.dispose();
          for (const [index, record] of native.pending.entries())
            if (!record.settled)
              native.respond(
                index,
                native.calls[index]?.type === 'surface.release' ? 'applied' : 'cancelled',
              );
          await released;
        };
        retirements.push(retire);
        return {
          native,
          surface,
          errors,
          retire,
          /** Answer every outstanding ticket; camera requests report the host's current camera. */
          async drain(next?: CanvasSurfaceViewportState): Promise<void> {
            if (next) camera = next;
            for (let guard = 0; guard < 24; guard++) {
              await settle();
              const index = native.pending.findIndex((record) => !record.settled);
              if (index < 0) return;
              const call = native.calls[index]!;
              native.respond(
                index,
                call.type === 'surface.prepare' ? 'ready' : 'applied',
                call.type === 'surface.viewport.request' ? camera : undefined,
              );
            }
            throw new Error('Native operations did not converge.');
          },
          async dispatch(command: CanvasCommand): Promise<string> {
            const outcome = surface.port.dispatch(
              surface.port.createRequest(command, surface.port.read().revision),
            );
            return (await outcome).status;
          },
          selections(): Extract<CanvasSurfaceCommand, { type: 'surface.select' }>[] {
            return native.calls.flatMap((call) => (call.type === 'surface.select' ? [call] : []));
          },
          viewportCommands() {
            return native.calls.flatMap((call) =>
              call.type === 'surface.viewport.request' ? [call.command] : [],
            );
          },
        };
      },
    };
  }
  return { workflow };
}

/** Select one source's first output through the shared command port. */
function slot(catalog: CanvasCatalogSnapshot, index: number): CanvasCommand {
  const source = catalog.entries[index]?.source;
  if (!source) throw new Error('Missing fixture source.');
  return { type: 'select', selection: { sourceId: source.id, batchIndex: 0 } };
}

/** Let the attachment answer a shared command while the host settles its native tickets. */
async function apply(
  surface: Awaited<ReturnType<ReturnType<ReturnType<typeof workspace>['workflow']>['mount']>>,
  command: CanvasCommand,
  camera?: CanvasSurfaceViewportState,
): Promise<void> {
  const outcome = surface.dispatch(command);
  await surface.drain(camera);
  await expect(outcome).resolves.toBe('applied');
}

test('a first Run starts at Fit, then other slots restore the linked settled camera', async () => {
  const workflow = workspace().workflow('linked');
  const f = workflow.mount();
  await f.drain();
  expect(f.surface.port.capabilities.commands).toContain('link');
  expect(f.selections()).toHaveLength(1);
  expect(f.selections()[0]).not.toHaveProperty('inspection');
  const settled = zoomed(0.25, 0.75);
  await apply(f, { type: 'zoom', factor: 2 }, settled);
  await apply(f, slot(workflow.catalog, 1), zoomed(0.6, 0.4, 0.5));
  expect(f.selections().at(-1)?.inspection).toEqual(restored(settled));
  await apply(f, slot(workflow.catalog, 0));
  expect(f.selections().at(-1)?.inspection).toEqual(restored(zoomed(0.6, 0.4, 0.5)));
  expect(f.errors).toEqual([]);
});

test('unlinked slots restore their own cameras and linking re-inspects the shown slot', async () => {
  const workflow = workspace().workflow('unlinked');
  const f = workflow.mount();
  await f.drain();
  await apply(f, slot(workflow.catalog, 0), zoomed(0.3, 0.3));
  await apply(f, { type: 'link', enabled: false });
  expect(f.viewportCommands().at(-1)).toEqual({
    type: 'inspect',
    inspection: restored(zoomed(0.3, 0.3)),
  });
  await apply(f, slot(workflow.catalog, 1), zoomed(0.8, 0.2, 4));
  expect(f.selections().at(-1)).not.toHaveProperty('inspection');
  await apply(f, slot(workflow.catalog, 0));
  expect(f.selections().at(-1)?.inspection).toEqual(restored(zoomed(0.3, 0.3)));
  await apply(f, slot(workflow.catalog, 1));
  expect(f.selections().at(-1)?.inspection).toEqual(restored(zoomed(0.8, 0.2, 4)));
  await apply(f, { type: 'link', enabled: true });
  expect(f.viewportCommands().at(-1)).toEqual({
    type: 'inspect',
    inspection: restored(zoomed(0.8, 0.2, 4)),
  });
  await apply(f, slot(workflow.catalog, 2));
  expect(f.selections().at(-1)?.inspection).toEqual(restored(zoomed(0.8, 0.2, 4)));
  expect(f.errors).toEqual([]);
});

test('unlinking a slot without its own camera fits it, and a Fit camera is never restored', async () => {
  const workflow = workspace().workflow('fit');
  const f = workflow.mount();
  await f.drain();
  await apply(f, { type: 'link', enabled: false }, fit());
  expect(f.viewportCommands().at(-1)).toEqual({ type: 'fit' });
  await apply(f, slot(workflow.catalog, 1));
  await apply(f, slot(workflow.catalog, 0));
  expect(f.selections().every((selection) => !('inspection' in selection))).toBe(true);
  expect(f.errors).toEqual([]);
});

test('a newer Run retires saved cameras and starts at Fit', async () => {
  const workflow = workspace().workflow('runs');
  const f = workflow.mount();
  await f.drain(zoomed(0.4, 0.6));
  workflow.run(2);
  f.surface.refresh();
  await f.drain(zoomed(0.1, 0.9));
  const fresh = f.selections().at(-1);
  expect(fresh?.file.filename).toBe('2.png');
  expect(fresh).not.toHaveProperty('inspection');
  await apply(f, slot(workflow.catalog, 1));
  expect(f.selections().at(-1)?.inspection).toEqual(restored(zoomed(0.1, 0.9)));
  expect(f.errors).toEqual([]);
});

test('each workflow restores its own camera when its tab returns', async () => {
  const shared = workspace();
  const first = shared.workflow('first');
  const second = shared.workflow('second');
  const a = first.mount();
  await a.drain(zoomed(0.2, 0.8, 3));
  await a.retire();
  const b = second.mount();
  await b.drain(zoomed(0.7, 0.7));
  expect(b.selections().at(-1)).not.toHaveProperty('inspection');
  await b.retire();
  const returned = first.mount();
  await returned.drain();
  expect(returned.selections().at(-1)?.inspection).toEqual(restored(zoomed(0.2, 0.8, 3)));
  await returned.retire();
  const back = second.mount();
  await back.drain();
  expect(back.selections().at(-1)?.inspection).toEqual(restored(zoomed(0.7, 0.7)));
  expect([...a.errors, ...b.errors, ...returned.errors, ...back.errors]).toEqual([]);
});

test('a host without the inspect command keeps selections at Fit and withholds linking', async () => {
  const workflow = workspace().workflow('plain');
  const f = workflow.mount({});
  await f.drain(zoomed(0.25, 0.25));
  await apply(f, slot(workflow.catalog, 1));
  await apply(f, slot(workflow.catalog, 0));
  expect(f.surface.port.capabilities.commands).not.toContain('link');
  expect(f.selections().every((selection) => !('inspection' in selection))).toBe(true);
  expect(f.errors).toEqual([]);
});
