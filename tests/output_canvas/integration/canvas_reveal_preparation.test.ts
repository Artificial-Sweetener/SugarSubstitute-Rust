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
/** Prove thumbnail inspection reuses bounded decode leases and refuses stale asynchronous completions. */
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasImageBudget } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageBudget.js';
import { CanvasImageAllowances } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageAllowances.js';
import { CanvasImageDecodeScheduler } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageDecodeScheduler.js';
import { CanvasImageResources } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageResources.js';
import { canvasDisplayPlan } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasDisplayPlan.js';
import { BrowserCanvasResources } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasResources.js';
import { BrowserCanvasSourceNavigation } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasSourceNavigation.js';
import { canvasRevealTarget } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasReveal.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { BrowserCanvasPlatformFixture } from '../support/BrowserCanvasPlatformFixture.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';

/** Compose real selection/resources with only the browser decoder held at an explicit boundary. */
function fixture() {
  let catalog = canvasCatalogFixture();
  const views = new CanvasViewRegistry(() => 'reveal-preparation');
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  selection.reconcile(view, catalog);
  const platform = new BrowserCanvasPlatformFixture();
  const clock = new WorkflowTestClock();
  const budget = new CanvasImageBudget();
  const resources = new CanvasImageResources({
    clock,
    budget,
    decodes: new CanvasImageDecodeScheduler(platform, clock),
    changed: () => {},
    logger: console,
  });
  const browser = new BrowserCanvasResources(
    resources,
    budget,
    new CanvasImageAllowances(),
    (item) => `http://localhost/view?filename=${item.file.filename}&id=${item.id}`,
  );
  let active = true;
  const plan = () =>
    canvasDisplayPlan(views.read(view), catalog, selection.items(view, catalog), null);
  browser.reconcile(plan());
  const navigation = new BrowserCanvasSourceNavigation({
    resources: browser,
    selection,
    read: () => (active ? { plan: plan(), catalog } : null),
    cells: () => [],
  });
  return {
    catalog,
    views,
    view,
    selection,
    platform,
    browser,
    navigation,
    replace: () => {
      catalog = canvasCatalogFixture(2);
      browser.reconcile(plan());
    },
    deactivate: () => {
      active = false;
      browser.active(false);
    },
    dispose: () => browser.dispose(),
  };
}

test('decodes the exact clicked final before leaving a grid and applies manual single-image selection', async () => {
  const f = fixture();
  try {
    f.selection.apply(f.view, f.catalog, { type: 'overview', enabled: true });
    const item = f.catalog.entries[0]!.items[0]!;
    const pending = f.navigation.reveal(canvasRevealTarget(item));
    expect(f.views.read(f.view).mode).toBe('overview');
    const index = f.platform.requests.findIndex((request) => request.input.key === item.id);
    expect(index).toBeGreaterThanOrEqual(0);
    await f.platform.ready(index);
    expect(await pending).toBe(true);
    expect(f.views.read(f.view)).toMatchObject({
      mode: 'image',
      follow: 'manual',
      sourcePreview: false,
      comparison: null,
      selection: { sourceId: item.provenance.sourceId, batchIndex: 0 },
    });
  } finally {
    f.dispose();
  }
});

test.each(['run', 'workflow'] as const)(
  'never applies a delayed image after a %s transition',
  async (change) => {
    const f = fixture();
    try {
      const item = f.catalog.entries[0]!.items[0]!;
      const pending = f.navigation.reveal(canvasRevealTarget(item));
      const index = f.platform.requests.findIndex((request) => request.input.key === item.id);
      if (change === 'run') f.replace();
      else f.deactivate();
      await f.platform.ready(index);
      expect(await pending).toBe(false);
      expect(f.views.read(f.view).follow).toBe('auto');
    } finally {
      f.dispose();
    }
  },
);
