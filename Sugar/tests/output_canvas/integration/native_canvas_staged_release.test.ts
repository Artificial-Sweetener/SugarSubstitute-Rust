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
/** Prove the page releases every staged native candidate it leaves, so the host's slot never deadlocks. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { CanvasPrimarySurface } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasPrimarySurface.js';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import type { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import {
  windowId,
  type CanvasViewId,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type { CanvasSurfaceCommand } from '../../../integrations/contracts/CanvasSurface.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { previewGeometry, settleNativePreview } from '../support/NativeCanvasPreviewFixture.js';
import { nativeCanvasStagingHost } from '../support/NativeCanvasStagingHost.js';

const disposals: (() => void)[] = [];
afterEach(() => {
  for (const dispose of disposals.splice(0)) dispose();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

/** Compose the real primary surface and native attachments over the one-slot staging host. */
function fixture(initial: CanvasCatalogSnapshot = canvasCatalogFixture()) {
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
  let catalog = initial;
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const first = views.create(catalog.sessionId).viewId;
  const second = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(first, catalog);
  selection.reconcile(second, catalog);
  let active: CanvasViewId = first;
  let renderer = 'native';
  const coordinator = {
    active: () => active,
    catalog: () => catalog,
    subscribe: () => () => undefined,
  } as unknown as CanvasSessionCoordinator;
  const element = document.createElement('div');
  document.body.append(element);
  const native = nativeCanvasStagingHost();
  const errors: unknown[] = [];
  const attachments: NativeCanvasAttachment[] = [];
  const clock = new WorkflowTestClock();
  const primary = new CanvasPrimarySurface({
    document,
    window: windowId('native-staged-release'),
    views,
    coordinator,
    element: () => element,
    renderer: () => renderer,
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
        nativeSurface: () => native.host,
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
    first,
    second,
    async settle(): Promise<void> {
      for (let turn = 0; turn < 8; turn++) await settleNativePreview();
    },
    switchTo(view: CanvasViewId): void {
      active = view;
      primary.reconcile();
    },
    /** Replace the renderer identity, which stages a replacement while keeping the current one. */
    rediscover(next: string): void {
      renderer = next;
      primary.reconcile();
    },
    /** Advance the active view's catalog generation, as a workflow restore does. */
    restore(next: CanvasCatalogSnapshot): void {
      catalog = next;
      selection.reconcile(first, next);
      primary.reconcile();
    },
    releases: (): CanvasSurfaceCommand[] =>
      native.calls.filter(({ type }) => type === 'surface.release'),
  };
}

test.each<CanvasSurfaceCommand['type']>(['surface.select', 'surface.geometry'])(
  'a startup canvas the host prepared but never committed is released when the page switches workflows (%s unanswered)',
  async (unanswered) => {
    const f = fixture();
    f.withheld.add(unanswered);
    f.primary.setup();
    await f.settle();
    const startup = f.attachments[0]!;
    const address = startup.port.read().address;
    expect(f.staged()).toBe(address.attachmentId);
    expect(f.committed()).toBeNull();
    f.switchTo(f.second);
    await f.settle();
    expect(f.releases().map((call) => call.address)).toEqual([address]);
    expect(f.staged()).not.toBe(address.attachmentId);
    f.finish(unanswered);
    await f.settle();
    const shown = f.attachments.at(-1)!;
    expect(shown.port.read().address.viewId).toBe(f.second);
    expect(f.primary.diagnostics().address).toEqual(shown.port.read().address);
    expect(f.committed()).toBe(shown.port.read().address.attachmentId);
    expect(f.held()).toEqual([]);
    expect(f.errors).toEqual([]);
  },
);

test.each<['renderer' | 'restore', 'empty' | 'ready']>([
  ['renderer', 'empty'],
  ['restore', 'ready'],
])(
  'a same-view replacement (%s) is not held behind a current canvas the host never committed',
  async (replacement, state) => {
    // No output to present: the committed view's canvas prepares and places, but never commits.
    const empty = canvasCatalogFixture(1, 0, []);
    const f = fixture(empty);
    f.primary.setup();
    await f.settle();
    const current = f.attachments[0]!;
    const address = current.port.read().address;
    expect(f.primary.diagnostics().address).toEqual(address);
    expect(f.staged()).toBe(address.attachmentId);
    expect(f.committed()).toBeNull();
    if (replacement === 'renderer') f.rediscover('native:rediscovered');
    else f.restore(canvasCatalogFixture(1, 1));
    await f.settle();
    const next = f.attachments.at(-1)!;
    expect(next).not.toBe(current);
    expect(next.port.read().address.generation).toBe(2);
    // The host answered the replacement's prepare: either the current one was released or it
    // committed. Neither attachment may wait behind the other.
    expect(f.held()).toEqual([]);
    expect(
      f.calls.some(
        (call) =>
          call.type === 'surface.release' && call.address.attachmentId === address.attachmentId,
      ),
    ).toBe(true);
    expect(f.primary.diagnostics().address).toEqual(next.port.read().address);
    expect(next.port.read().resource.state).toBe(state);
    expect(f.errors).toEqual([]);
  },
);
