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
import {
  NativeCanvasAttachment,
  type NativeCanvasAttachmentOptions,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import {
  canvasOverviewPlacements,
  canvasOverviewWindow,
} from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOverviewWindow.js';
import {
  canvasOutputCatalog,
  canvasSceneCatalog,
} from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
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
function fixture(
  capabilities: Partial<CanvasSurfaceCapabilities>,
  actions: Partial<Pick<NativeCanvasAttachmentOptions, 'fileAction' | 'drag'>> = {},
  build: () => ReturnType<typeof batchCatalog> = batchCatalog,
) {
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
  const catalog = build();
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
    report: (error) => errors.push(error),
    ...actions,
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
    views,
    view,
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

/** Deliver a synchronous dragstart with a recording DataTransfer, as the page's engine would. */
function dragStart(element: HTMLElement) {
  const values = new Map<string, string>();
  const files: File[] = [];
  const transfer = {
    setData: (type: string, value: string) => values.set(type, value),
    items: { add: (file: File) => files.push(file), clear: () => files.splice(0) },
    effectAllowed: 'none',
  };
  const event = new MouseEvent('dragstart', { bubbles: true, cancelable: true });
  Object.defineProperty(event, 'dataTransfer', { value: transfer });
  element.dispatchEvent(event);
  return { event, values, files };
}

test('painted overview cells drag out their exact prepared file; other cells refuse', async () => {
  const drag = jest.fn<NonNullable<NativeCanvasAttachmentOptions['drag']>>(async (item) => ({
    file: new File(['png'], item.file.filename, { type: 'image/png' }),
    url: `http://localhost/view?filename=${item.file.filename}`,
  }));
  const f = fixture({ overview: true }, { drag });
  await f.drain();
  const [first, , temporary] = f.catalog.entries[0]!.items;
  const cells = [...f.root.querySelectorAll<HTMLButtonElement>('.sc-output-overview-cell')];
  const image = cells[0]!.querySelector<HTMLElement>('.sc-output-overview-cell-status')!;
  image.dispatchEvent(new MouseEvent('pointerover', { bubbles: true }));
  await settle();
  expect(drag).toHaveBeenCalledWith(first, expect.any(AbortSignal));
  expect(cells[0]!.draggable).toBe(true);
  const dragged = dragStart(cells[0]!);
  expect(dragged.event.defaultPrevented).toBe(false);
  expect(dragged.files.map((file) => file.name)).toEqual([first!.file.filename]);
  expect(dragged.values.get('DownloadURL')).toBe(
    `image/png:${first!.file.filename}:http://localhost/view?filename=${first!.file.filename}`,
  );
  cells[2]!.dispatchEvent(new MouseEvent('pointerover', { bubbles: true }));
  await settle();
  expect(drag).not.toHaveBeenCalledWith(temporary, expect.anything());
  expect(cells[0]!.draggable).toBe(false);
  expect(dragStart(cells[2]!).event.defaultPrevented).toBe(true);
  expect(dragStart(cells[3]!).event.defaultPrevented).toBe(true);
  expect(f.errors).toEqual([]);
});

test('Open media on the native presenter runs the shared file action for the exact video', async () => {
  const accepted: boolean[] = [];
  const fileAction = jest.fn<NonNullable<NativeCanvasAttachmentOptions['fileAction']>>(
    async (_item, accepts) => {
      accepted.push(accepts());
      return true;
    },
  );
  const f = fixture({ overview: true }, { fileAction });
  await f.drain();
  expect(f.surface.port.capabilities.commands).toContain('open-media');
  const video = f.catalog.entries[0]!.items[3]!;
  f.root.querySelectorAll<HTMLButtonElement>('.sc-output-overview-cell')[3]!.click();
  await f.drain();
  expect(f.surface.port.read().selected?.id).toBe(video.id);
  const open = window.open;
  const opened = jest.fn<typeof window.open>();
  window.open = opened;
  try {
    expect(await f.dispatch({ type: 'open-media', target: 'primary' })).toBe('applied');
  } finally {
    window.open = open;
  }
  expect(fileAction).toHaveBeenCalledWith(video, expect.any(Function), expect.any(AbortSignal));
  expect(accepted).toEqual([true]);
  expect(opened).not.toHaveBeenCalled();
  expect(f.errors).toEqual([]);
});

test('the native presenter hides Open media without a shared file action', async () => {
  const f = fixture({ overview: true });
  await f.drain();
  expect(f.surface.port.capabilities.commands).not.toContain('open-media');
  expect(await f.dispatch({ type: 'open-media', target: 'primary' })).toBe('unsupported');
});

/** Two prompt scenes of one scene run, each with its own two-image batch, viewed on All. */
function sceneCatalog() {
  const batch = batchCatalog();
  const output = batch.result.outputs[0]!;
  const sceneOutput = (scene: string, run: number) => ({
    ...output,
    items: output.items.slice(0, 2).map((item) => ({
      ...item,
      id: `${item.id}:${scene}` as typeof item.id,
      dimensions: null,
      provenance: { ...item.provenance, runOrder: run },
      file: { ...item.file, filename: `${scene}.png` },
    })),
  });
  const result = {
    ...batch.result,
    runOrder: 2,
    scenes: {
      id: 'scene-run',
      running: null,
      scenes: [
        { key: 'morning', title: 'Morning', order: 0, outputs: [sceneOutput('morning', 1)] },
        { key: 'night', title: 'Night', order: 1, outputs: [sceneOutput('night', 2)] },
      ],
    },
  } as WorkflowResultSession;
  return canvasSceneCatalog(result, batch.sources, null, null);
}

test('All paints one natively drawn cell per scene, and a cell opens its scene', async () => {
  const f = fixture({ overview: true }, {}, sceneCatalog);
  f.views.update(f.view, (state) => ({ ...state, mode: 'overview', scene: null }));
  await f.dispatch({ type: 'select-scene', scene: null });
  await f.drain();
  const painted = f.overviews().at(-1)?.cells ?? [];
  expect(painted.map((cell) => cell.file?.filename)).toEqual(['morning.png', 'night.png']);
  expect(painted[0]!.rect.width).toBeGreaterThan(240);
  expect(painted[1]!.rect.x - painted[0]!.rect.x - painted[0]!.rect.width).toBeLessThan(3);
  const cells = [...f.root.querySelectorAll<HTMLButtonElement>('.sc-output-overview-cell')];
  expect(cells.map((cell) => cell.getAttribute('aria-label'))).toEqual(['Morning', 'Night']);
  cells[1]!.click();
  expect(f.views.read(f.view)).toMatchObject({ scene: 'night', follow: 'manual' });
  expect(f.errors).toEqual([]);
});
