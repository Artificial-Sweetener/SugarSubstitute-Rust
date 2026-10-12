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
/** Exercise the composed browser surface and reusable Vue overlay against actual state owners. */

import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { BrowserCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasAttachment.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasImageAllowances } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageAllowances.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import type { CanvasCommand } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { BrowserCanvasPlatformFixture } from '../support/BrowserCanvasPlatformFixture.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';

/** Allocate real owners and replace only DOM measurement and platform decoding. */
function fixture(brokenUrl = false) {
  const bounds = jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    width: 800,
    height: 600,
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 800,
    bottom: 600,
    toJSON: () => ({}),
  });
  let catalog: CanvasCatalogSnapshot = canvasCatalogFixture();
  let keys = 0;
  const views = new CanvasViewRegistry(() => String(++keys));
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views);
  selection.reconcile(view, catalog);
  selection.apply(view, catalog, {
    type: 'select-source',
    selection: { sourceId: catalog.entries[0]!.source.id, batchIndex: 0 },
  });
  const address = views.prepareAttachment(view, windowId('browser'));
  views.commitAttachment(address);
  const root = document.createElement('section');
  document.body.append(root);
  const clock = new WorkflowTestClock();
  const frames = new CanvasFrameFixture();
  const loader = new BrowserCanvasPlatformFixture();
  const inspections = new CanvasInspectionStore();
  const allowances = new CanvasImageAllowances();
  const errors: unknown[] = [];
  const options = {
    address,
    views,
    selection,
    inspections,
    allowances,
    clock,
    frames,
    loader,
    logger: {
      error: (...args: unknown[]) => {
        errors.push(args);
      },
    },
    catalog: () => catalog,
    accepts: () => views.accepts(address) && catalog.generation === 0,
    connected: () => true,
    url: (item: { readonly id: string }) => {
      if (brokenUrl) throw new Error('File URL is unavailable.');
      return `${document.location.origin}/view?id=${item.id}`;
    },
    open: jest.fn(),
    disposeLoader: () => loader.dispose(),
    report: (error: unknown) => {
      errors.push(error);
    },
  };
  let sequence = 0;
  /** Submit a control intent from its current qualified port revision. */
  const command = async (surface: BrowserCanvasAttachment, intent: CanvasCommand) =>
    surface.port.dispatch({
      schema: 1,
      address,
      requestId: ++sequence,
      expectedRevision: surface.port.read().revision,
      command: intent,
    });
  /** Change the authoritative catalog at a named result boundary. */
  const replace = (next: CanvasCatalogSnapshot) => {
    catalog = next;
  };
  return {
    bounds,
    catalog,
    views,
    view,
    selection,
    address,
    root,
    clock,
    frames,
    loader,
    inspections,
    allowances,
    errors,
    options,
    command,
    replace,
  };
}

describe('composed browser canvas attachment', () => {
  afterEach(() => {
    document.body.replaceChildren();
    jest.restoreAllMocks();
  });

  test('mounts full-fidelity admitted handles and controls with literal source labels and actual zoom status', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    expect(surface.port.read().resource.state).toBe('loading');
    const image = await f.loader.ready(0);
    f.clock.advance(100);
    f.frames.advance(16);
    await nextTick();
    expect(f.root.querySelector('img')).toBe(image);
    expect(f.root.textContent).toContain('<b>Output</b>');
    expect(f.root.querySelector('b')).toBeNull();
    expect(surface.port.read().resource.state).toBe('ready');
    expect(surface.port.read().zoom.dimensions).toEqual({ width: 3840, height: 2160 });
    expect((await f.command(surface, { type: 'native' })).status).toBe('applied');
    f.frames.advance(45);
    f.frames.advance(16);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('native');
    expect(image.style.transform).toContain('scale(1)');
    surface.dispose();
    expect(f.loader.requests[0]?.released).toBe(true);
    expect(f.loader.disposed).toBe(true);
    expect(f.root.children).toHaveLength(0);
    expect(f.views.accepts(f.address)).toBe(false);
    expect(f.frames.pending).toBe(0);
    expect(f.errors).toEqual([]);
  });

  test('keeps independent camera routing on the actually displayed outgoing image until replacement', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    await f.loader.ready(0);
    f.clock.advance(100);
    await f.command(surface, { type: 'link', enabled: false });
    await f.command(surface, { type: 'native' });
    f.frames.advance(45);
    await f.command(surface, {
      type: 'select',
      selection: { sourceId: f.catalog.entries[1]!.source.id, batchIndex: 0 },
    });
    await f.command(surface, { type: 'zoom', factor: 2 });
    f.frames.advance(45);
    await f.loader.ready(1);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('fit');
    await f.command(surface, {
      type: 'select',
      selection: { sourceId: f.catalog.entries[0]!.source.id, batchIndex: 0 },
    });
    await f.loader.ready(2);
    f.clock.advance(100);
    expect(surface.port.read().zoom.physicalScale).toBe(2);
    surface.dispose();
  });

  test('suspends zero-sized surfaces, refuses hidden preview interest, and restores light view state', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    await f.loader.ready(0);
    await f.command(surface, { type: 'native' });
    f.frames.advance(45);
    f.bounds.mockReturnValue({
      width: 0,
      height: 0,
      x: 0,
      y: 0,
      left: 0,
      top: 0,
      right: 0,
      bottom: 0,
      toJSON: () => ({}),
    });
    window.dispatchEvent(new Event('resize'));
    f.frames.advance(16);
    expect(surface.diagnostics().displayed).toBe(0);
    expect(
      surface.wants(f.catalog.result.context.sessionId, 0, f.catalog.entries[0]!.source.id),
    ).toBe(false);
    f.bounds.mockReturnValue({
      width: 800,
      height: 600,
      x: 0,
      y: 0,
      left: 0,
      top: 0,
      right: 800,
      bottom: 600,
      toJSON: () => ({}),
    });
    window.dispatchEvent(new Event('resize'));
    f.frames.advance(16);
    await f.loader.ready(1);
    f.clock.advance(100);
    expect(surface.port.read().zoom.mode).toBe('native');
    surface.dispose();
    expect(f.frames.pending).toBe(0);
  });

  test('contains late completion and stale commands after attachment retirement', async () => {
    const f = fixture();
    const surface = new BrowserCanvasAttachment(f.root, f.options);
    surface.dispose();
    await f.loader.ready(0);
    expect(f.loader.requests[0]?.released).toBe(true);
    expect(f.root.querySelector('img')).toBeNull();
    expect((await f.command(surface, { type: 'native' })).status).toBe('cancelled');
    expect(surface.diagnostics().pendingDecodes).toBe(0);
    expect(f.frames.pending).toBe(0);
  });

  test('cleans mounted controls, observers, and the lease when initial composition fails', () => {
    const f = fixture(true);
    expect(() => new BrowserCanvasAttachment(f.root, f.options)).toThrow(
      'File URL is unavailable.',
    );
    expect(f.loader.disposed).toBe(true);
    expect(f.root.children).toHaveLength(0);
    expect(f.views.accepts(f.address)).toBe(false);
    expect(f.frames.pending).toBe(0);
  });
});
