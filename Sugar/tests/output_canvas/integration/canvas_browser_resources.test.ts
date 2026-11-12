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
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
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
  test.each(['final', 'next-run'])(
    'retires an overview preview at %s before deferred cell layout',
    async (transition) => {
      const f = fixture();
      const preview = f.preview(1);
      const entry = f.catalog.entries[0]!;
      const current: CanvasCatalogSnapshot = {
        ...f.catalog,
        result: { ...f.catalog.result, runOrder: 2, runStatus: 'running', preview },
        entries: [{ ...entry, items: [], preview }],
      };
      f.views.update(f.view, (state) => ({ ...state, mode: 'overview' }));
      f.browser.reconcile(f.plan(current, preview));
      f.browser.overview([
        {
          key: 'preview',
          selection: { sourceId: entry.source.id, batchIndex: 0 },
          label: 'Preview',
          item: null,
          preview: true,
        },
      ]);
      f.browser.receive(preview, new Blob(['preview']));
      await f.ready(0, { width: 512, height: 288 });
      if (transition === 'final')
        f.browser.reconcile(f.plan(canvasCatalogFixture(2, 0, [entry.source])));
      else {
        const next = { ...preview, promptId: 'prompt:next' as const, runOrder: 3, token: 'next' };
        const catalog: CanvasCatalogSnapshot = {
          ...current,
          result: { ...current.result, runOrder: 3, presentedRunOrder: 3, preview: next },
          entries: [{ ...entry, items: [], preview: next }],
        };
        f.browser.reconcile(f.plan(catalog, next));
      }
      expect(f.resources.read()).toEqual([]);
      expect(f.loads[0]?.released).toBe(true);
      f.browser.dispose();
    },
  );

  test.each([0, 2])(
    'moves a visible same-run preview from overview position %i without awaiting another frame',
    async (index) => {
      const f = fixture();
      const preview = f.preview(1);
      const entry = f.catalog.entries[0]!;
      const current: CanvasCatalogSnapshot = {
        ...f.catalog,
        result: { ...f.catalog.result, runOrder: 2, runStatus: 'running', preview },
        entries: [{ ...entry, items: [], preview }],
      };
      const cell = {
        key: 'preview-cell',
        selection: { sourceId: entry.source.id, batchIndex: 0 },
        label: 'Preview',
        item: null,
        preview: true,
      };
      f.views.update(f.view, (state) => ({
        ...state,
        mode: 'overview',
        selection: { sourceId: entry.source.id, batchIndex: index },
      }));
      f.browser.reconcile(f.plan(current, preview));
      f.browser.overview([cell]);
      f.browser.receive(preview, new Blob(['preview']));
      await f.ready(0, { width: 512, height: 288 });
      const frame = f.resources.read()[0]!.frame;
      expect(frame?.selection.batchIndex).toBe(0);
      f.views.update(f.view, (state) => ({ ...state, mode: 'image', selection: cell.selection }));
      f.browser.reconcile(f.plan(current, preview));
      expect(f.resources.read()[0]?.frame).toBe(frame);
      expect(f.resources.read()[0]?.slot).toBe('primary');
      expect(f.browser.previewStarted).toBe(true);
      f.views.update(f.view, (state) => ({ ...state, mode: 'overview' }));
      f.browser.reconcile(f.plan(current, preview));
      f.browser.overview([cell]);
      expect(f.resources.read()[0]?.frame).toBe(frame);
      expect(f.resources.read()[0]?.slot).toBe(cell.key);
      expect(f.loads).toHaveLength(1);
      const next = { ...preview, promptId: 'prompt:next' as const, runOrder: 3, token: 'new-run' };
      f.views.update(f.view, (state) => ({ ...state, mode: 'image' }));
      f.browser.reconcile(f.plan(current, next));
      expect(f.resources.read()).toEqual([]);
      expect(f.loads[0]?.released).toBe(true);
      f.browser.dispose();
    },
  );

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

  test('releases an outgoing image before displaying controls for another source', async () => {
    const f = fixture();
    f.browser.reconcile(f.plan());
    await f.ready(0);
    f.selection.apply(f.view, f.catalog, {
      type: 'select',
      selection: { sourceId: f.catalog.entries[1]!.source.id, batchIndex: 0 },
    });
    f.browser.reconcile(f.plan());
    expect(f.resources.read()[0]?.frame).toBeNull();
    expect(f.loads[0]?.released).toBe(true);
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
