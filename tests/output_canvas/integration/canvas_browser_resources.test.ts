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
/** Prove catalog and byte delivery compose without restarting same-run previews or retaining hidden pixels. */

import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasImageBudget } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageBudget.js';
import { CanvasImageAllowances } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageAllowances.js';
import { CanvasImageDecodeScheduler } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageDecodeScheduler.js';
import { CanvasImageResources } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageResources.js';
import { canvasDisplayPlan } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasDisplayPlan.js';
import { BrowserCanvasResources } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasResources.js';
import type { BrowserCanvasImagePayload } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasImageLoader.js';
import type {
  CanvasDecodedImage,
  CanvasImageInput,
  CanvasImageLoadPort,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasImageResource.js';
import type { ImageDimensions } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type { WorkflowPreview } from '../../../frontend/comfyui/ui/execution/domain/WorkflowPreview.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { settleCanvasImages } from '../support/CanvasImageFixture.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';

interface PlatformLoad {
  readonly input: CanvasImageInput<BrowserCanvasImagePayload>;
  readonly resolve: (image: CanvasDecodedImage<HTMLImageElement>) => void;
  released: boolean;
}

/** Control only platform decoding while using the actual navigation, resource, and allowance owners. */
function fixture() {
  const catalog = canvasCatalogFixture();
  const views = new CanvasViewRegistry(() => 'browser-resources');
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views);
  selection.reconcile(view, catalog);
  selection.apply(view, catalog, {
    type: 'select-source',
    selection: { sourceId: catalog.entries[0]!.source.id, batchIndex: 0 },
  });
  const clock = new WorkflowTestClock();
  const budget = new CanvasImageBudget();
  const allowances = new CanvasImageAllowances();
  const loads: PlatformLoad[] = [];
  const loader: CanvasImageLoadPort<BrowserCanvasImagePayload, HTMLImageElement> = {
    /** Supply a controllable decode lease, retaining diagnostics only in this test boundary. */
    start(input) {
      let resolve: PlatformLoad['resolve'] = () => {
        throw new Error('Decode is uninitialized.');
      };
      const completion = new Promise<CanvasDecodedImage<HTMLImageElement>>((ready) => {
        resolve = ready;
      });
      const load = { input, resolve, released: false };
      loads.push(load);
      return {
        completion,
        cancel: () => {
          load.released = true;
        },
      };
    },
  };
  const decodes = new CanvasImageDecodeScheduler(loader, clock);
  const errors: unknown[] = [];
  const resources = new CanvasImageResources({
    clock,
    budget,
    decodes,
    changed: () => {},
    logger: {
      error: (...args: unknown[]) => {
        errors.push(args);
      },
    },
  });
  const browser = new BrowserCanvasResources(
    resources,
    budget,
    allowances,
    (item) => `http://localhost/view?filename=${item.file.filename}&v=${item.id}`,
  );
  /** Project a current authoritative selection with optional transient metadata. */
  const plan = (current: CanvasCatalogSnapshot = catalog, preview: WorkflowPreview | null = null) =>
    canvasDisplayPlan(views.read(view), current, selection.items(view, current), preview);
  /** Complete one platform load with observable release authority. */
  const ready = async (
    index: number,
    dimensions: ImageDimensions = { width: 3840, height: 2160 },
  ) => {
    const load = loads[index]!;
    const handle = document.createElement('img');
    load.resolve({
      handle,
      dimensions,
      release: () => {
        load.released = true;
      },
    });
    await settleCanvasImages();
    browser.measured();
  };
  /** Correlate a newer execution's unknown-batch source-level frame. */
  const preview = (sequence: number): WorkflowPreview => ({
    sessionId: catalog.result.context.sessionId,
    generation: catalog.generation,
    promptId: 'prompt:preview',
    sourceId: catalog.entries[0]!.source.id,
    runOrder: 2,
    sequence,
    token: `preview:${sequence}`,
    dimensions: null,
  });
  return {
    catalog,
    views,
    view,
    selection,
    clock,
    budget,
    allowances,
    loads,
    resources,
    browser,
    plan,
    ready,
    preview,
    errors,
  };
}

describe('browser resource composition', () => {
  test('does not restart pending same-run previews for metadata/camera/wipe changes and finals bypass their cadence', async () => {
    const f = fixture();
    f.browser.reconcile(f.plan());
    await f.ready(0);
    const blob = new Blob(['preview']);
    f.browser.reconcile(f.plan(f.catalog, f.preview(1)));
    f.browser.receive(f.preview(1), blob);
    expect(f.loads).toHaveLength(2);
    for (let frame = 2; frame <= 100; frame++) {
      f.browser.reconcile(f.plan(f.catalog, f.preview(frame)));
      f.browser.receive(f.preview(frame), blob);
    }
    expect(f.loads).toHaveLength(2);
    expect(f.loads[1]?.released).toBe(false);
    expect(f.resources.diagnostics().queuedInputs).toBe(1);
    await f.ready(1);
    expect(f.resources.read()[0]?.frame?.preview).toBe(true);
    const final = canvasCatalogFixture(
      2,
      0,
      f.catalog.entries.slice(0, 1).map((entry) => entry.source),
    );
    f.selection.reconcile(f.view, final);
    f.browser.reconcile(f.plan(final));
    expect(f.loads).toHaveLength(3);
    expect(f.loads[2]?.input.preview).toBe(false);
    expect(f.loads[1]?.released).toBe(true);
    await f.ready(2);
    expect(f.resources.read()[0]?.frame?.key).toBe(final.entries[0]?.items[0]?.id);
    f.browser.dispose();
    expect(f.loads.every((load) => load.released)).toBe(true);
    expect(f.resources.diagnostics().estimatedBytes).toBe(0);
  });

  test('retains the actual outgoing slot route during ordinary navigation and releases it on source deletion', async () => {
    const f = fixture();
    f.browser.reconcile(f.plan());
    await f.ready(0);
    const original = f.resources.read()[0]?.frame;
    f.selection.apply(f.view, f.catalog, {
      type: 'select',
      selection: { sourceId: f.catalog.entries[1]!.source.id, batchIndex: 0 },
    });
    f.browser.reconcile(f.plan());
    expect(f.resources.read()[0]?.frame).toBe(original);
    const deleted = { ...f.catalog, entries: f.catalog.entries.slice(1) };
    f.browser.reconcile(f.plan(deleted));
    expect(f.resources.read()[0]?.frame).toBeNull();
    expect(f.loads[0]?.released).toBe(true);
    f.browser.dispose();
  });

  test('releases every prior-generation frame before a same-ID workflow replacement', async () => {
    const f = fixture();
    f.browser.reconcile(f.plan());
    await f.ready(0);
    const replacement = canvasCatalogFixture(1, 1);
    f.selection.reconcile(f.view, replacement);
    f.browser.reconcile(f.plan(replacement));
    expect(f.loads[0]?.released).toBe(true);
    expect(f.resources.read().every((slot) => slot.frame === null)).toBe(true);
    f.browser.dispose();
  });

  test('approves measured unknown finals without retaining rejected handles and recovers scoped geometry', async () => {
    const f = fixture();
    const unknown = {
      ...f.catalog,
      entries: f.catalog.entries.map((entry) => ({
        ...entry,
        items: entry.items.map((item) => ({ ...item, dimensions: null })),
      })),
    };
    f.browser.reconcile(f.plan(unknown));
    await f.ready(0, { width: 16384, height: 8192 });
    expect(f.loads[0]?.released).toBe(true);
    expect(f.resources.read()[0]?.status.state).toBe('budget');
    f.browser.admit();
    expect(f.loads).toHaveLength(2);
    await f.ready(1, { width: 16384, height: 8192 });
    expect(f.resources.read()[0]?.status.state).toBe('ready');
    expect(f.allowances.dimensions(f.view, 0, unknown.entries[0]!.items[0]!.id)).toEqual({
      width: 16384,
      height: 8192,
    });
    f.browser.dispose();
    expect(f.budget.estimatedBytes).toBe(0);
    expect(f.errors).toEqual([]);
  });
});
