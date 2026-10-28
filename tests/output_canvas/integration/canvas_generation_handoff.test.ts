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
/** Characterize attribution from generation through the active canvas attachment. */

import { jest } from '@jest/globals';
import { WorkflowCanvasCoordinator } from '../../../frontend/comfyui/ui/execution/application/WorkflowCanvasCoordinator.js';
import { WorkflowExecutionIngestor } from '../../../frontend/comfyui/ui/execution/application/WorkflowExecutionIngestor.js';
import { WorkflowGenerationEvents } from '../../../frontend/comfyui/ui/execution/application/WorkflowGenerationEvents.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { promptId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { ComfyWorkflowPreviewDelivery } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowPreviewDelivery.js';
import { CanvasImageAllowances } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageAllowances.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import { CanvasSourceInventory } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSourceInventory.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { WebCanvasAdapter } from '../../../frontend/comfyui/ui/output_canvas/application/WebCanvasAdapter.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { BrowserCanvasAttachment } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasAttachment.js';
import { CanvasPrimarySurface } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasPrimarySurface.js';
import { workflowFinal, workflowResultsFixture } from '../../execution/support/workflow_results.js';
import { BrowserCanvasPlatformFixture } from '../support/BrowserCanvasPlatformFixture.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';

test('background results stay with their workflow and late decodes cannot replace the active canvas', async () => {
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    width: 800,
    height: 600,
    right: 800,
    bottom: 600,
    toJSON: () => ({}),
  });
  const f = workflowResultsFixture();
  const sessions = new WorkflowSessionStore();
  const contexts = [f.a.session, f.b.session];
  sessions.synchronize(contexts, f.a.session.sessionId);
  const inventory = new CanvasSourceInventory();
  inventory.replace(f.a.session, f.a.sources);
  inventory.replace(f.b.session, f.b.sources);
  let key = 0;
  const views = new CanvasViewRegistry(() => String(++key));
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  const inspections = new CanvasInspectionStore();
  const allowances = new CanvasImageAllowances();
  const frames = new CanvasFrameFixture();
  const loaders: BrowserCanvasPlatformFixture[] = [];
  const errors: unknown[] = [];
  const report = (error: unknown): void => {
    errors.push(error);
  };
  const events = new WorkflowGenerationEvents(report);
  const delivery = new WorkflowCanvasCoordinator<Blob>(events, f.results, report, () => ['web']);
  const web: WebCanvasAdapter<Blob> = new WebCanvasAdapter<Blob>({
    wants: (frame): boolean => primary.wants(frame.source),
    receive: (frame) => primary.receive(frame.preview, frame.bytes),
  });
  delivery.register(web.adapter);
  const coordinator: CanvasSessionCoordinator = new CanvasSessionCoordinator(
    sessions,
    web,
    inventory,
    views,
    selection,
    { subscribe: () => () => undefined },
  );
  coordinator.install();
  const root = document.createElement('div');
  document.body.append(root);
  const primary: CanvasPrimarySurface = new CanvasPrimarySurface({
    document,
    views,
    coordinator,
    window: windowId('primary'),
    element: () => root,
    renderer: () => 'vue',
    report,
    mount: (element, address, _catalog, accepts) => {
      const loader = new BrowserCanvasPlatformFixture();
      loaders.push(loader);
      return new BrowserCanvasAttachment(element, {
        address,
        views,
        selection,
        inspections,
        allowances,
        clock: f.clock,
        frames,
        loader,
        logger: { error: report },
        accepts,
        catalog: () => coordinator.catalog(address.viewId),
        connected: () => true,
        url: (item) => `${document.location.origin}/view?filename=${item.file.filename}`,
        open: () => undefined,
        disposeLoader: () => loader.dispose(),
        report,
      });
    },
  });
  primary.setup();
  const ingestor = new WorkflowExecutionIngestor({
    runs: f.runs,
    results: f.results,
    clock: f.clock,
    logger: { warn: report },
    recover: () => undefined,
    generation: events,
  });
  const previews = new ComfyWorkflowPreviewDelivery(f.results, { error: report }, (frame) =>
    delivery.preview(frame),
  );
  try {
    const a = promptId('job-a');
    const b = promptId('job-b');
    ingestor.accepted(ingestor.submitting(f.a), a);
    ingestor.accepted(ingestor.submitting(f.b), b);
    const sourceA = ingestor.resolveSource(a, ['42:23']);
    const sourceB = ingestor.resolveSource(b, ['42:23']);
    if (!sourceA || !sourceB) throw new Error('Missing attributed workflow sources.');

    previews.deliver(sourceB, new Blob(['background']));
    expect(loaders[0]?.requests).toHaveLength(0);
    previews.deliver(sourceA, new Blob(['foreground']));
    expect(loaders[0]?.requests[0]?.input.preview).toBe(true);
    await loaders[0]?.ready(0);

    ingestor.final(workflowFinal(b, 'background.png'));
    ingestor.final(workflowFinal(a, 'foreground.png'));
    expect(coordinator.catalog(coordinator.active()!)?.entries[0]?.items[0]?.file.filename).toBe(
      'foreground.png',
    );
    expect(loaders[0]?.requests[1]?.input.payload).toMatchObject({
      kind: 'file',
      url: expect.stringContaining('foreground.png'),
    });

    sessions.synchronize(contexts, f.b.session.sessionId);
    expect(loaders[0]?.disposed).toBe(true);
    expect(loaders[1]?.requests[0]?.input.payload).toMatchObject({
      kind: 'file',
      url: expect.stringContaining('background.png'),
    });
    await loaders[0]?.ready(1);
    expect(loaders[0]?.requests[1]?.released).toBe(true);
    await loaders[1]?.ready(0);
    expect(root.querySelector('img')?.src).toContain('background.png');

    sessions.synchronize(contexts, f.a.session.sessionId);
    expect(loaders[1]?.disposed).toBe(true);
    expect(loaders[2]?.requests[0]?.input.payload).toMatchObject({
      kind: 'file',
      url: expect.stringContaining('foreground.png'),
    });
    await loaders[2]?.ready(0);
    expect(root.querySelector('img')?.src).toContain('foreground.png');
    expect(errors).toEqual([]);
  } finally {
    previews.dispose();
    primary.dispose();
    coordinator.dispose();
    delivery.dispose();
    web.dispose();
    ingestor.dispose();
    sessions.dispose();
    root.remove();
    jest.restoreAllMocks();
  }
});
