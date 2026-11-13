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
/** Prove a replacement attachment of an overview view presents the view's overview once it commits. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { CanvasPrimarySurface } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasPrimarySurface.js';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { nativeCanvasRendererKey } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasCapabilities.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { canvasOutputCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import type { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import {
  promptId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type { WorkflowResultSession } from '../../../frontend/comfyui/ui/execution/domain/WorkflowResultSession.js';
import type { CanvasSurfaceHost } from '../../../integrations/contracts/CanvasSurface.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasSources } from '../support/CanvasCatalogFixture.js';
import { previewGeometry, settleNativePreview } from '../support/NativeCanvasPreviewFixture.js';
import { nativeCanvasStagingHost } from '../support/NativeCanvasStagingHost.js';

const disposals: (() => void)[] = [];
afterEach(() => {
  for (const dispose of disposals.splice(0)) dispose();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

/** One source with a batch of `count` images at a catalog generation; four open as an overview. */
function batchCatalog(generation: number, count = 4): CanvasCatalogSnapshot {
  const [source] = canvasSources();
  if (!source) throw new Error('Missing source.');
  const context = { sessionId: workflowSessionId('restored'), generation, title: '', path: '' };
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
  const items: WorkflowOutputItem[] = [0, 1, 2, 3].slice(0, count).map((batchIndex) => ({
    id: `result:restored-${generation}-${batchIndex}`,
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
 * Compose the real primary surface over a one-slot host that paints overviews and previews.
 * `undiscovered` mounts the first canvas while the desktop's native discovery is still pending,
 * keyed as the Comfy host keys a native renderer, so concluding discovery changes the renderer.
 */
function fixture(options: { readonly undiscovered?: boolean } = {}) {
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
  // As in a browser, a staged attachment's hidden root measures sized but not visible.
  jest.spyOn(CanvasSurfaceGeometryObserver.prototype, 'read').mockImplementation(function (
    this: CanvasSurfaceGeometryObserver,
  ) {
    const { element } = this as unknown as { readonly element: HTMLElement };
    return element.closest('[style*="visibility: hidden"]')
      ? { ...previewGeometry, visible: false, clip: { x: 0, y: 0, width: 0, height: 0 } }
      : previewGeometry;
  });
  let catalog = batchCatalog(0);
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(view, catalog);
  let renderer = 'native';
  const native = nativeCanvasStagingHost({ overview: true, preview: true });
  let surface: CanvasSurfaceHost | null = options.undiscovered ? null : native.host;
  const coordinator = {
    active: () => view,
    catalog: () => catalog,
    subscribe: () => () => undefined,
  } as unknown as CanvasSessionCoordinator;
  const element = document.createElement('div');
  document.body.append(element);
  const errors: unknown[] = [];
  const attachments: NativeCanvasAttachment[] = [];
  const clock = new WorkflowTestClock();
  const primary = new CanvasPrimarySurface({
    document,
    window: windowId('native-replacement-overview'),
    views,
    coordinator,
    element: () => element,
    renderer: () =>
      options.undiscovered
        ? `litegraph:${nativeCanvasRendererKey(surface)}:${surface ? 'native' : 'pending'}`
        : renderer,
    retireBeforeMount: (previous, next) =>
      previous.endsWith(':browser') !== next.endsWith(':browser'),
    report: (error) => errors.push(error),
    mount: (root, address, _catalog, accepts, acceptsPreparation) => {
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
        nativeSurface: () => surface,
        surfacePending: () => surface === null,
        connected: () => true,
        report: (error) => errors.push(error),
      });
      attachments.push(attachment);
      return attachment;
    },
  });
  disposals.push(() => primary.dispose());
  return {
    ...native,
    primary,
    attachments,
    errors,
    async settle(): Promise<void> {
      for (let turn = 0; turn < 8; turn++) await settleNativePreview();
    },
    replace(kind: 'renderer' | 'restore', outputs = 4): void {
      if (kind === 'renderer') renderer = 'native:rediscovered';
      else {
        catalog = batchCatalog(1, outputs);
        selection.reconcile(view, catalog);
      }
      primary.reconcile();
    },
    /** Conclude native discovery with the overview-painting surface, without reconciling. */
    discover(): void {
      surface = native.host;
    },
    /** Deliver the restored generation's outputs, as result restoration does after the graph. */
    deliver(): void {
      catalog = batchCatalog(1);
      selection.reconcile(view, catalog);
      primary.reconcile();
    },
  };
}

test('a restored replacement mounted before its outputs commits an empty canvas, then presents their overview', async () => {
  const f = fixture();
  f.primary.setup();
  await f.settle();
  // The restored generation arrives before its outputs: nothing to show, so no select or overview.
  f.replace('restore', 0);
  await f.settle();
  const fresh = f.attachments.at(-1)!;
  const address = fresh.port.read().address;
  expect(f.primary.diagnostics().address).toEqual(address);
  expect(fresh.port.read().view.mode).toBe('image');
  expect(f.of(address.attachmentId)).toEqual([
    'surface.prepare',
    'surface.geometry',
    'surface.commit',
  ]);
  f.deliver();
  await f.settle();
  expect(f.attachments.at(-1)).toBe(fresh);
  expect(fresh.port.read().view.mode).toBe('overview');
  expect(f.of(address.attachmentId)).toEqual([
    'surface.prepare',
    'surface.geometry',
    'surface.commit',
    'surface.overview',
  ]);
  expect(fresh.viewport.classList.contains('sc-output-native')).toBe(true);
  expect(f.errors).toEqual([]);
});

test.each<['renderer' | 'restore', 'acquiring' | 'shown']>([
  ['renderer', 'acquiring'],
  ['renderer', 'shown'],
  ['restore', 'acquiring'],
  ['restore', 'shown'],
])(
  'a %s replacement of an overview view whose first canvas is %s presents the overview',
  async (kind, first) => {
    const f = fixture();
    if (first === 'acquiring') f.withheld.add('surface.overview');
    f.primary.setup();
    await f.settle();
    const outdated = f.attachments[0]!;
    expect(outdated.port.read().view.mode).toBe('overview');
    expect(f.of(outdated.port.read().address.attachmentId)).toContain('surface.overview');
    f.replace(kind);
    await f.settle();
    if (first === 'acquiring') f.finish('surface.overview');
    await f.settle();
    const fresh = f.attachments.at(-1)!;
    const address = fresh.port.read().address;
    expect(address.generation).toBe(2);
    expect(f.primary.diagnostics().address).toEqual(address);
    expect(f.of(address.attachmentId)).toEqual(
      expect.arrayContaining(['surface.prepare', 'surface.overview', 'surface.commit']),
    );
    expect(f.committed()).toBe(address.attachmentId);
    expect(fresh.port.read().view.mode).toBe('overview');
    expect(fresh.viewport.classList.contains('sc-output-native')).toBe(true);
    expect(f.errors).toEqual([]);
  },
);

/**
 * A restored overview view mounts its canvas while native discovery is pending, so that canvas
 * has no overview painter and could only commit an empty image. Concluding discovery changes the
 * renderer and stages a replacement that paints the overview. A first canvas the host never
 * committed is released before the replacement stages; one already shown is kept until the
 * replacement has presented the overview.
 */
test.each<['unbound' | 'preparing' | 'committed']>([['unbound'], ['preparing'], ['committed']])(
  'a restored overview canvas mounted before native discovery is replaced by one that presents the overview when it was %s',
  async (first) => {
    const f = fixture({ undiscovered: true });
    f.primary.setup();
    await f.settle();
    const early = f.attachments[0]!;
    const earlyId = early.port.read().address.attachmentId;
    // Without an overview painter the first canvas can present only image detail.
    expect(early.port.read().view.mode).toBe('image');
    expect(f.calls).toEqual([]);
    // As in the relaunch, the replacement's overview is still being acquired while the first
    // canvas is refreshed again.
    f.withheld.add('surface.overview');
    f.discover();
    if (first !== 'unbound') early.refresh();
    if (first === 'committed') {
      await f.settle();
      expect(f.committed()).toBe(earlyId);
      expect(f.of(earlyId)).not.toContain('surface.overview');
    }
    f.primary.reconcile();
    await f.settle();
    // A later catalog or view notification refreshes whatever the surface still holds.
    f.primary.reconcile();
    await f.settle();
    f.finish('surface.overview');
    await f.settle();
    expect(f.attachments).toHaveLength(2);
    const fresh = f.attachments[1]!;
    const address = fresh.port.read().address;
    expect(f.primary.diagnostics().address).toEqual(address);
    expect(f.committed()).toBe(address.attachmentId);
    expect(f.staged()).toBeNull();
    expect(f.held()).toEqual([]);
    expect(f.of(address.attachmentId)).toEqual(
      expect.arrayContaining(['surface.prepare', 'surface.overview', 'surface.commit']),
    );
    expect(f.of(address.attachmentId)).not.toContain('surface.release');
    const step = (type: string, id: string): number =>
      f.calls.findIndex((call) => call.type === type && call.address.attachmentId === id);
    if (first === 'unbound') expect(f.of(earlyId)).toEqual([]);
    else if (first === 'preparing')
      expect(step('surface.release', earlyId)).toBeLessThan(
        step('surface.prepare', address.attachmentId),
      );
    else
      expect(step('surface.release', earlyId)).toBeGreaterThan(
        step('surface.overview', address.attachmentId),
      );
    expect(fresh.port.read().view.mode).toBe('overview');
    expect(fresh.viewport.classList.contains('sc-output-native')).toBe(true);
    expect(f.errors).toEqual([]);
  },
);
