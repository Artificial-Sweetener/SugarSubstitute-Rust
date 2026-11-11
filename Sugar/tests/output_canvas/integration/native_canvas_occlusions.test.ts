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
/** Prove any web content covering a native viewport is published for host-side gesture routing. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasAttachment.js';
import { ensureBrowserCanvasStyles } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasStyles.js';
import { NativeCanvasOcclusions } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasOcclusions.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
} from '../../../integrations/contracts/CanvasSurface.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import type { CanvasEditHost } from '../../../integrations/contracts/CanvasEditing.js';
import {
  CanvasEditHostFixture,
  editState,
} from '../../canvas_editing/support/CanvasEditHostFixture.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import {
  nativePreviewHost,
  finalViewport,
  settleNativePreview as settle,
} from '../support/NativeCanvasPreviewFixture.js';

/**
 * The native host chooses every cursor over its canvas from the canvas's interaction state, so
 * the page sets none on the viewport, even beside a browser-rendered canvas's styles.
 */
function expectHostOwnedCursor(viewport: HTMLElement): void {
  ensureBrowserCanvasStyles(document);
  expect(getComputedStyle(viewport).cursor).not.toMatch(/grab/);
}

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
  Reflect.deleteProperty(document, 'elementsFromPoint');
});

interface Box {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

/** The viewport's client box; every test box is expressed in the same client space. */
const viewportBox: Box = { left: 100, top: 50, width: 400, height: 300 };

/**
 * Stand in for the layout and hit testing jsdom lacks: explicit client boxes, and a paint stack
 * placing elements marked as painted above the viewport before it and every other box after it.
 */
function layout() {
  const boxes = new Map<Element, Box>();
  const above = new Set<Element>();
  const surface = () => document.querySelector('[data-sc-canvas-geometry]');
  const boxOf = (element: Element): Box | undefined => {
    const own = boxes.get(element);
    if (own) return own;
    const viewport = surface();
    return viewport && (element === viewport || element.contains(viewport))
      ? viewportBox
      : undefined;
  };
  jest.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: Element,
  ) {
    const box = boxOf(this) ?? { left: 0, top: 0, width: 0, height: 0 };
    return {
      x: box.left,
      y: box.top,
      left: box.left,
      top: box.top,
      right: box.left + box.width,
      bottom: box.top + box.height,
      width: box.width,
      height: box.height,
      toJSON: () => ({}),
    };
  });
  const contains = (box: Box | undefined, x: number, y: number) =>
    !!box && x >= box.left && x < box.left + box.width && y >= box.top && y < box.top + box.height;
  Object.defineProperty(document, 'elementsFromPoint', {
    configurable: true,
    value: (x: number, y: number): Element[] => {
      const viewport = surface();
      const hit = [...document.querySelectorAll('*')].filter(
        (element) =>
          contains(boxOf(element), x, y) && (element as HTMLElement).style.pointerEvents !== 'none',
      );
      const covering = hit.filter((element) => above.has(element)).reverse();
      const rest = hit.filter((element) => !above.has(element) && element !== viewport).reverse();
      return [...covering, ...(viewport && hit.includes(viewport) ? [viewport] : []), ...rest];
    },
  });
  /** Mount a positioned element with an explicit box, painted above or behind the viewport. */
  const place = (
    parent: Element,
    box: Box,
    { painted = 'above', position = 'fixed', className = 'acme-popup' } = {},
  ): HTMLElement => {
    const element = document.createElement('div');
    element.className = className;
    if (position) element.style.position = position;
    boxes.set(element, box);
    if (painted === 'above') above.add(element);
    parent.append(element);
    return element;
  };
  return { boxes, place };
}

/** Build a bare attachment root around an identified viewport for direct measurements. */
function surfaceRoot() {
  const root = document.createElement('section');
  const viewport = document.createElement('div');
  viewport.setAttribute('data-sc-canvas-geometry', '');
  root.append(viewport);
  document.body.append(root);
  return { root, occlusions: new NativeCanvasOcclusions(root, viewport) };
}

const measure = (occlusions: NativeCanvasOcclusions) =>
  occlusions.read({ x: 100, y: 50 }, { width: 400, height: 300 });

test('detects an unknown third-party popup painted over the viewport', () => {
  const { place } = layout();
  const { occlusions } = surfaceRoot();
  place(document.body, { left: 450, top: 20, width: 160, height: 120 });
  // An in-flow element over the same area is not an overlay candidate.
  place(document.body, { left: 120, top: 60, width: 50, height: 50 }, { position: '' });
  expect(measure(occlusions)).toEqual({
    occlusions: [{ x: 350, y: 0, width: 50, height: 90 }],
    modal: false,
  });
});

test('ignores a positioned popup painted behind the viewport and a hidden one', () => {
  const { place } = layout();
  const { occlusions } = surfaceRoot();
  place(document.body, { left: 150, top: 80, width: 100, height: 100 }, { painted: 'behind' });
  const hidden = place(document.body, { left: 300, top: 100, width: 100, height: 100 });
  hidden.style.display = 'none';
  expect(measure(occlusions)).toEqual({ occlusions: [], modal: false });
});

test('reports the attachment chrome explicitly and a pass-through layer only by its controls', () => {
  const { place } = layout();
  const { root, occlusions } = surfaceRoot();
  place(root, { left: 108, top: 300, width: 200, height: 40 }, { className: 'sc-output-tabstrip' });
  const layer = place(document.body, { left: 0, top: 0, width: 1024, height: 768 });
  layer.style.pointerEvents = 'none';
  // A static control of the pass-through layer sits under the layer's centre sample.
  place(layer, { left: 250, top: 150, width: 100, height: 100 }, { position: '' });
  expect(measure(occlusions)).toEqual({
    occlusions: [
      { x: 8, y: 250, width: 200, height: 40 },
      { x: 150, y: 100, width: 100, height: 100 },
    ],
    modal: false,
  });
});

test('reports a modal dialog and a covering element spanning the viewport as modal', () => {
  const { place } = layout();
  const { occlusions } = surfaceRoot();
  const dialog = document.createElement('dialog');
  dialog.setAttribute('open', '');
  dialog.setAttribute('aria-modal', 'true');
  document.body.append(dialog);
  expect(measure(occlusions).modal).toBe(true);
  dialog.remove();
  expect(measure(occlusions).modal).toBe(false);
  place(document.body, { left: 0, top: 0, width: 1024, height: 768 });
  expect(measure(occlusions)).toEqual({
    occlusions: [{ x: 0, y: 0, width: 400, height: 300 }],
    modal: true,
  });
});

test('merges rects past the protocol ceiling into one covering rect', () => {
  const { place } = layout();
  const { root, occlusions } = surfaceRoot();
  for (let index = 0; index < 70; index++)
    place(
      root,
      { left: 100 + index * 4, top: 50, width: 2, height: 2 + index },
      { className: 'sc-output-status' },
    );
  const { occlusions: rects } = measure(occlusions);
  expect(rects).toHaveLength(64);
  expect(rects?.at(-1)).toEqual({ x: 252, y: 0, width: 26, height: 71 });
});

/** Mount real view, selection, port, geometry and presentation owners over a held host. */
function fixture(
  capabilities: Partial<CanvasSurfaceCapabilities>,
  editing: CanvasEditHost | null = null,
) {
  const scene = layout();
  // jsdom cannot measure visibility; keep the real measurement, including routing members.
  const read = CanvasSurfaceGeometryObserver.prototype.read;
  jest.spyOn(CanvasSurfaceGeometryObserver.prototype, 'read').mockImplementation(function (
    this: CanvasSurfaceGeometryObserver,
  ) {
    return {
      ...read.call(this),
      visible: true,
      clip: { x: 100, y: 50, width: 400, height: 300 },
    };
  });
  const catalog = canvasCatalogFixture();
  let sequence = 0;
  const views = new CanvasViewRegistry(() => String(++sequence));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 1 });
  selection.reconcile(view, catalog);
  const address = views.prepareAttachment(view, windowId('native-occlusion'));
  views.commitAttachment(address);
  const root = document.createElement('section');
  document.body.append(root);
  const native = nativePreviewHost(false);
  const host = { ...native.host, capabilities: { ...native.host.capabilities, ...capabilities } };
  const frames = new CanvasFrameFixture();
  const errors: unknown[] = [];
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
    editing: () => editing,
    connected: () => true,
    report: (error) => errors.push(error),
  });
  return {
    ...native,
    ...scene,
    surface,
    errors,
    /** Run pending frames and acknowledge every outstanding native ticket. */
    async drain(): Promise<void> {
      for (let guard = 0; guard < 32; guard++) {
        await settle();
        frames.advance(16);
        await settle();
        const index = native.pending.findIndex((record) => !record.settled);
        if (index < 0 && !frames.pending) return;
        if (index < 0) continue;
        const call = native.calls[index]!;
        native.respond(
          index,
          call.type === 'surface.prepare' ? 'ready' : 'applied',
          call.type === 'surface.viewport.request' ? finalViewport : undefined,
        );
      }
      throw new Error('Native operations did not converge.');
    },
    geometries(): Extract<CanvasSurfaceCommand, { type: 'surface.geometry' }>['geometry'][] {
      return native.calls.flatMap((call) =>
        call.type === 'surface.geometry' ? [call.geometry] : [],
      );
    },
    async dispose(): Promise<void> {
      const retired = surface.dispose();
      for (const [index, record] of native.pending.entries())
        if (!record.settled)
          native.respond(
            index,
            native.calls[index]?.type === 'surface.release' ? 'applied' : 'cancelled',
          );
      await retired;
    },
  };
}

test('publishes one acknowledged update per popup open, move and close, and none while unchanged', async () => {
  const f = fixture({ input: true });
  try {
    await f.drain();
    expect(f.geometries().at(-1)).toMatchObject({ occlusions: [], modal: false });
    expectHostOwnedCursor(f.surface.viewport);
    const popup = f.place(document.body, { left: 300, top: 100, width: 150, height: 80 });
    await f.drain();
    expect(f.geometries().at(-1)).toMatchObject({
      occlusions: [{ x: 200, y: 50, width: 150, height: 80 }],
      modal: false,
    });
    const opened = f.geometries().length;
    f.boxes.set(popup, { left: 320, top: 120, width: 150, height: 80 });
    popup.style.left = '320px';
    await f.drain();
    expect(f.geometries()).toHaveLength(opened + 1);
    expect(f.geometries().at(-1)).toMatchObject({
      occlusions: [{ x: 220, y: 70, width: 150, height: 80 }],
    });
    // A mutation that leaves the covered area identical sends nothing.
    popup.classList.add('is-open');
    await f.drain();
    expect(f.geometries()).toHaveLength(opened + 1);
    popup.remove();
    await f.drain();
    expect(f.geometries()).toHaveLength(opened + 2);
    expect(f.geometries().at(-1)).toMatchObject({ occlusions: [], modal: false });
    expect(f.calls.filter(({ type }) => type === 'surface.select')).toHaveLength(1);
    expect(f.errors).toEqual([]);
  } finally {
    await f.dispose();
  }
});

test('a host without input receives no routing members and observes no overlays', async () => {
  const f = fixture({});
  try {
    await f.drain();
    const sent = f.geometries();
    expect(sent.length).toBeGreaterThan(0);
    for (const geometry of sent) {
      expect(geometry).not.toHaveProperty('occlusions');
      expect(geometry).not.toHaveProperty('modal');
    }
    expectHostOwnedCursor(f.surface.viewport);
    f.place(document.body, { left: 300, top: 100, width: 150, height: 80 });
    await f.drain();
    expect(f.geometries()).toHaveLength(sent.length);
  } finally {
    await f.dispose();
  }
});

test('a routing host installs no pan or zoom handlers on the native viewport', async () => {
  const f = fixture({ input: true });
  try {
    await f.drain();
    const viewport = f.surface.viewport;
    const events = [
      new MouseEvent('pointerdown', { bubbles: true, cancelable: true, button: 0 }),
      new WheelEvent('wheel', { bubbles: true, cancelable: true, deltaY: -100 }),
      new MouseEvent('dblclick', { bubbles: true, cancelable: true }),
    ];
    for (const event of events) viewport.dispatchEvent(event);
    expect(events.map((event) => event.defaultPrevented)).toEqual([false, false, false]);
    expect(f.errors).toEqual([]);
  } finally {
    await f.dispose();
  }
});

test('mounts editing chrome for an editing host and routes gestures around it', async () => {
  const editing = new CanvasEditHostFixture(editState({ tool: 'brush' }));
  const f = fixture({ input: true }, editing);
  try {
    await f.drain();
    const rail = f.surface.viewport.parentElement?.querySelector('.sc-canvas-edit-rail');
    expect(rail).not.toBeNull();
    expect(editing.commands).toEqual([{ type: 'state' }]);
    f.boxes.set(rail!, { left: 108, top: 102, width: 40, height: 200 });
    rail!.classList.add('measured');
    await f.drain();
    expect(f.geometries().at(-1)?.occlusions).toContainEqual({
      x: 8,
      y: 52,
      width: 40,
      height: 200,
    });
    // Without an editable document the chrome withdraws and stops covering the canvas.
    editing.push({ document: null });
    await f.drain();
    expect(f.surface.viewport.parentElement?.querySelector('.sc-canvas-edit-rail')).toBeNull();
    expect(f.geometries().at(-1)?.occlusions).toEqual([]);
    expect(f.errors).toEqual([]);
  } finally {
    await f.dispose();
  }
  expect(editing.observers).toBe(0);
});

test('mounts no editing chrome for a canvas without an editing host', async () => {
  const f = fixture({ input: true });
  try {
    await f.drain();
    expect(f.surface.viewport.parentElement?.querySelector('.sc-canvas-edit-chrome')).toBeNull();
  } finally {
    await f.dispose();
  }
});
