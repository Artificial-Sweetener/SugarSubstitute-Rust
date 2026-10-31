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
/** Drive the shared overview grid through real view owners onto a fake native host. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import {
  canvasOverviewPlacements,
  canvasOverviewWindow,
} from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOverviewWindow.js';
import { canvasOutputCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type { CanvasCommand } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import type { CanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import {
  promptId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type { WorkflowResultSession } from '../../../frontend/comfyui/ui/execution/domain/WorkflowResultSession.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
} from '../../../integrations/contracts/CanvasSurface.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { canvasSources } from '../support/CanvasCatalogFixture.js';
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

/** The overview element and the viewport share this client box. */
const bounds = { left: 40, top: 30, width: 500, height: 400 };
/** Move only the overview region, as a host layout change between frames would. */
const overviewShift = { x: 0 };
const surfaceGeometry: CanvasSurfaceGeometry = {
  ...previewGeometry,
  origin: { x: bounds.left, y: bounds.top },
  clip: { x: bounds.left, y: bounds.top, width: bounds.width, height: bounds.height },
};

/** One source with two saved PNGs, a temporary PNG and a video. */
function batchCatalog() {
  const [source] = canvasSources();
  if (!source) throw new Error('Missing source.');
  const context = {
    sessionId: workflowSessionId('overview'),
    generation: 0,
    title: 'overview',
    path: '',
  };
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
  const items: WorkflowOutputItem[] = [
    ['ComfyUI_0.png', 'output', 'image'],
    ['ComfyUI_1.PNG', 'output', 'image'],
    ['ComfyUI_2.png', 'temp', 'image'],
    ['ComfyUI_3.mp4', 'output', 'video'],
  ].map(([filename, type, mediaKind], batchIndex) => ({
    id: `result:overview-${batchIndex}`,
    provenance,
    batchIndex,
    file: { filename: filename!, subfolder: '', type: type as 'output' | 'temp' },
    mediaKind: mediaKind as 'image' | 'video',
    dimensions: { width: 3840, height: 2160 },
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

/** Mount real view, selection, port and presentation owners over a held native host. */
function fixture(capabilities: Partial<CanvasSurfaceCapabilities>) {
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    const left =
      bounds.left + (this.classList.contains('sc-output-overview') ? overviewShift.x : 0);
    return {
      x: left,
      y: bounds.top,
      left,
      top: bounds.top,
      right: left + bounds.width,
      bottom: bounds.top + bounds.height,
      width: bounds.width,
      height: bounds.height,
      toJSON: () => ({}),
    };
  });
  // jsdom cannot measure visibility; keep the observed allocation equal to the surface geometry.
  jest.spyOn(CanvasSurfaceGeometryObserver.prototype, 'read').mockReturnValue(surfaceGeometry);
  const catalog = batchCatalog();
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(view, catalog);
  const address = views.prepareAttachment(view, windowId('native-overview'));
  views.commitAttachment(address);
  const root = document.createElement('section');
  document.body.append(root);
  const native = nativePreviewHost(false);
  const host = { ...native.host, capabilities: { ...native.host.capabilities, ...capabilities } };
  const errors: unknown[] = [];
  const frames = new CanvasFrameFixture();
  const surface = new NativeCanvasAttachment(root, {
    address,
    clock: new WorkflowTestClock(),
    views,
    selection,
    frames,
    inspections: new CanvasInspectionStore(),
    catalog: () => catalog,
    accepts: () => views.accepts(address),
    acceptsPreparation: () => false,
    nativeSurface: () => host,
    connected: () => true,
    open: () => {},
    report: (error) => errors.push(error),
  });
  surface.port.updateSurface(address, surfaceGeometry);
  retirements.push(async () => {
    const retired = surface.dispose();
    for (const [index, record] of native.pending.entries())
      if (!record.settled)
        native.respond(
          index,
          native.calls[index]?.type === 'surface.release' ? 'applied' : 'cancelled',
        );
    await retired;
  });
  return {
    ...native,
    root,
    surface,
    frames,
    catalog,
    address,
    errors,
    /** Acknowledge every outstanding native ticket, answering camera requests with state. */
    async drain(): Promise<void> {
      for (let guard = 0; guard < 16; guard++) {
        await settle();
        const index = native.pending.findIndex((record) => !record.settled);
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
    async dispatch(command: CanvasCommand): Promise<string> {
      const outcome = surface.port.dispatch(
        surface.port.createRequest(command, surface.port.read().revision),
      );
      return (await outcome).status;
    },
    overviews(): Extract<CanvasSurfaceCommand, { type: 'surface.overview' }>[] {
      return native.calls.flatMap((call) => (call.type === 'surface.overview' ? [call] : []));
    },
  };
}

/** Lay out the four-image grid exactly as the shared overview window places it. */
function placements(catalog: ReturnType<typeof batchCatalog>) {
  return canvasOverviewPlacements(
    canvasOverviewWindow(catalog, bounds.width, bounds.height, 0, { width: 3840, height: 2160 }),
  );
}

test('a host without overview keeps image presentation, the command set and no grid', async () => {
  const f = fixture({});
  await f.drain();
  expect(f.surface.port.capabilities.commands).not.toContain('overview');
  const snapshot = f.surface.port.read();
  expect(snapshot.view.mode).toBe('image');
  expect(snapshot.grid).toEqual([]);
  expect(f.root.querySelector('.sc-output-overview')).toBeNull();
  expect(f.root.classList.contains('sc-output-overview-mode')).toBe(false);
  expect(f.overviews()).toEqual([]);
  expect(f.calls.some(({ type }) => type === 'surface.select')).toBe(true);
  expect(f.errors).toEqual([]);
});

test('enters a multi-image batch as a natively painted grid with the browser controls', async () => {
  const f = fixture({ overview: true });
  await f.drain();
  expect(f.surface.port.capabilities.commands).toContain('overview');
  const snapshot = f.surface.port.read();
  expect(snapshot.view.mode).toBe('overview');
  expect(snapshot.grid.map((entry) => entry.items.length)).toEqual([4]);
  expect(snapshot.resource.state).toBe('empty');
  expect(f.root.classList.contains('sc-output-overview-mode')).toBe(true);
  const overview = f.root.querySelector<HTMLElement>('.sc-output-overview')!;
  expect(overview.hidden).toBe(false);
  expect(overview.getAttribute('role')).toBe('region');
  const buttons = [...overview.querySelectorAll('button')];
  expect(buttons.map((button) => button.getAttribute('aria-label'))).toEqual(
    [1, 2, 3].map((index) => `<b>Output</b> · Image ${index}`).concat('<b>Output</b> · Video 4'),
  );
  expect(
    buttons.map((button) => button.querySelector('.sc-output-overview-cell-status')?.textContent),
  ).toEqual(['', '', 'Select to view this image.', 'Select to open this media.']);
  expect(buttons[3]?.querySelector('.sc-output-media-badge')?.textContent).toBe('Video');
  const expected = placements(f.catalog);
  const [first, second] = f.catalog.entries[0]!.items;
  const painted = f.overviews().at(-1)?.cells ?? [];
  expect(painted.map(({ itemId, file }) => ({ itemId, file }))).toEqual([
    { itemId: first!.id, file: first!.file },
    { itemId: second!.id, file: second!.file },
  ]);
  expectRect(painted[0]?.rect, expected[0]!);
  expectRect(painted[1]?.rect, expected[1]!);
  expect(f.calls.map(({ type }) => type)).toEqual([
    'surface.prepare',
    'surface.geometry',
    'surface.overview',
    'surface.commit',
  ]);
  expect(f.errors).toEqual([]);
});

/** Compare a wire rect with its expected placement within floating layout tolerance. */
function expectRect(
  actual: { x: number; y: number; width: number; height: number } | undefined,
  expected: { x: number; y: number; width: number; height: number },
): void {
  if (!actual) throw new Error('Missing painted cell.');
  for (const key of ['x', 'y', 'width', 'height'] as const)
    expect(actual[key]).toBeCloseTo(expected[key], 6);
}

test('converts full cell rects to the surface origin without clipping them', async () => {
  const f = fixture({ overview: true });
  await f.drain();
  const expected = placements(f.catalog);
  f.surface.port.updateSurface(f.address, {
    ...surfaceGeometry,
    origin: { x: bounds.left - 10, y: bounds.top - 10 },
  });
  await f.drain();
  const [first, second] = f.overviews().at(-1)!.cells;
  expectRect(first?.rect, {
    x: expected[0]!.x + 10,
    y: expected[0]!.y + 10,
    width: expected[0]!.width,
    height: expected[0]!.height,
  });
  // The second column extends past the viewport; the host clips the full rect it receives.
  expectRect(second?.rect, { ...expected[1]!, x: expected[1]!.x + 10, y: expected[1]!.y + 10 });
  expect(second!.rect.x + second!.rect.width).toBeGreaterThan(bounds.width);
  expect(f.calls.some(({ type }) => type === 'surface.viewport.request')).toBe(false);
});

test('coalesces a scroll burst into one layout pass and one resent overview', async () => {
  const f = fixture({ overview: true });
  await f.drain();
  const sent = f.overviews().length;
  const expected = placements(f.catalog);
  overviewShift.x = -20;
  try {
    const overview = f.root.querySelector<HTMLElement>('.sc-output-overview')!;
    overview.dispatchEvent(new Event('scroll'));
    const scheduled = f.frames.pending;
    overview.dispatchEvent(new Event('scroll'));
    overview.dispatchEvent(new Event('scroll'));
    expect(f.frames.pending).toBe(scheduled);
    f.frames.advance(16);
    expect(f.frames.pending).toBe(0);
    await f.drain();
    expect(f.overviews()).toHaveLength(sent + 1);
    const [first, second] = f.overviews().at(-1)!.cells;
    // The first column now starts left of the surface and keeps its full rect.
    expectRect(first?.rect, { ...expected[0]!, x: expected[0]!.x - 20 });
    expect(first!.rect.x).toBeLessThan(0);
    expectRect(second?.rect, { ...expected[1]!, x: expected[1]!.x - 20 });
    overview.dispatchEvent(new Event('scroll'));
    f.frames.advance(16);
    await f.drain();
    expect(f.overviews()).toHaveLength(sent + 1);
  } finally {
    overviewShift.x = 0;
  }
});

test('a cell click selects its result as an ordinary image and leaves the overview', async () => {
  const f = fixture({ overview: true });
  await f.drain();
  const second = f.catalog.entries[0]!.items[1]!;
  const button = f.root.querySelectorAll<HTMLButtonElement>('.sc-output-overview-cell')[1]!;
  button.click();
  await f.drain();
  expect(f.surface.port.read().view.mode).toBe('image');
  expect(f.surface.port.read().view.selection?.batchIndex).toBe(1);
  expect(f.root.querySelector<HTMLElement>('.sc-output-overview')!.hidden).toBe(true);
  expect(f.root.classList.contains('sc-output-overview-mode')).toBe(false);
  const selected = f.calls.findIndex((call) => call.type === 'surface.select');
  expect(f.calls[selected]).toMatchObject({ itemId: second.id, file: second.file });
  expect(f.calls[selected]).not.toHaveProperty('comparison');
  expect(f.calls[selected + 1]).toMatchObject({
    type: 'surface.viewport.request',
    command: { type: 'read' },
  });
  expect(await f.dispatch({ type: 'overview', enabled: true })).toBe('applied');
  await f.drain();
  expect(f.surface.port.read().view.mode).toBe('overview');
  expect(f.calls.at(-1)?.type).toBe('surface.overview');
  expect(f.errors).toEqual([]);
});
