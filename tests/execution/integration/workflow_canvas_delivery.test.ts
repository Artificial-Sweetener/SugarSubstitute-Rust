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
/** Prove live generation delivery targets a peer canvas independently of output presentation. */
import { jest } from '@jest/globals';
import { WorkflowCanvasCoordinator } from '../../../frontend/comfyui/ui/execution/application/WorkflowCanvasCoordinator.js';
import { documentCanvasAdapter } from '../../../frontend/comfyui/ui/execution/infrastructure/DocumentCanvasAdapter.js';
import { WebCanvasAdapter } from '../../../frontend/comfyui/ui/output_canvas/application/WebCanvasAdapter.js';
import type { WorkflowCanvasAdapter } from '../../../frontend/comfyui/ui/execution/domain/WorkflowCanvas.js';
import type { CanvasGenerationImages } from '../../../frontend/comfyui/ui/execution/domain/CanvasGenerationImages.js';
import { WorkflowGenerationEvents } from '../../../frontend/comfyui/ui/execution/application/WorkflowGenerationEvents.js';
import { WorkflowExecutionIngestor } from '../../../frontend/comfyui/ui/execution/application/WorkflowExecutionIngestor.js';
import {
  promptId,
  outputSourceId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { CanvasLayerHost } from '../../../integrations/contracts/CanvasHost.js';
import type { CanvasMaskHost } from '../../../integrations/contracts/CanvasMasks.js';
import type {
  CanvasContext,
  CanvasInsertImage,
  CanvasInsertResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { workflowFinal, workflowResultsFixture } from '../support/workflow_results.js';

/** Assemble real attribution and delivery owners with only native and image-byte boundaries replaced. */
function fixture() {
  const f = workflowResultsFixture();
  const report = jest.fn<(error: unknown) => void>();
  const events = new WorkflowGenerationEvents(report);
  const context: CanvasContext = {
    canvas: {
      id: 'editor',
      label: 'Editor',
      capabilities: { displayOutput: false, insertImageLayer: true },
    },
    document: { id: 'original', geometryRevision: 'original-size', width: 512, height: 384 },
  };
  const readContext = jest.fn<() => Promise<CanvasContext>>().mockResolvedValue(context);
  const insert = jest
    .fn<(image: CanvasInsertImage) => Promise<CanvasInsertResult>>()
    .mockResolvedValue({
      status: 'inserted',
      documentId: 'original',
      layerId: 'new-layer',
      left: -128,
      top: -64,
      width: 768,
      height: 512,
    });
  const canvas: CanvasLayerHost = {
    readCanvas: () => context.canvas,
    readContext,
    insertImage: insert,
  };
  const image = { format: 'png' as const, encoding: 'base64' as const, data: 'png-bytes' };
  const load = jest.fn<CanvasGenerationImages['load']>().mockResolvedValue(image);
  const targets = ['editor', 'web', 'failing-peer'];
  const delivery = new WorkflowCanvasCoordinator<Blob>(events, f.results, report, () => targets);
  delivery.register(documentCanvasAdapter('editor', canvas, { load }, report));
  const frames: Blob[] = [];
  const web = new WebCanvasAdapter<Blob>({
    wants: () => true,
    receive: (frame) => {
      frames.push(frame.bytes);
    },
  });
  delivery.register(web.adapter);
  const ingestor = new WorkflowExecutionIngestor({
    runs: f.runs,
    results: f.results,
    clock: f.clock,
    logger: console,
    recover: () => {},
    generation: events,
  });
  return {
    ...f,
    report,
    events,
    context,
    readContext,
    insert,
    canvas,
    load,
    delivery,
    ingestor,
    web,
    frames,
    targets,
  };
}

test('discovers mask operations by peer capability and refuses an ambiguous default', () => {
  const f = fixture();
  expect(f.delivery.maskHost()).toBeNull();
  expect(f.web.adapter.capabilities.masks).toBeUndefined();
  const masks: CanvasMaskHost = {
    canvasId: 'editor',
    execute: async () => ({ kind: 'context', supported: true, document: f.context.document }),
    subscribe: () => () => {},
  };
  Object.defineProperty(f.canvas, 'masks', { get: () => masks });
  expect(f.delivery.maskHost()).toBe(masks);
  expect(f.delivery.maskHost('editor')).toBe(masks);
  const other: CanvasMaskHost = { ...masks, canvasId: 'future-editor' };
  const unregister = f.delivery.register(
    documentCanvasAdapter(
      'future-editor',
      { ...f.canvas, masks: other },
      { load: f.load },
      f.report,
    ),
  );
  expect(f.delivery.maskHost()).toBeNull();
  expect(f.delivery.maskHost('future-editor')).toBe(other);
  unregister();
  expect(f.delivery.maskHost()).toBe(masks);
  f.delivery.dispose();
});

test('updates the web peer immediately while the document peer is transferring an image', async () => {
  const f = fixture();
  let finishTransfer:
    | ((image: Awaited<ReturnType<CanvasGenerationImages['load']>>) => void)
    | undefined;
  let transferring: (() => void) | undefined;
  const transferStarted = new Promise<void>((resolve) => {
    transferring = resolve;
  });
  f.load.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finishTransfer = resolve;
        transferring?.();
      }),
  );
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  f.ingestor.started(f.id);
  f.ingestor.final(workflowFinal(f.id));
  expect(f.web.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('final.png');
  expect(f.web.read(f.a.session.sessionId)?.runStatus).toBe('running');
  await transferStarted;
  expect(f.insert).not.toHaveBeenCalled();
  f.ingestor.final(workflowFinal(f.id, 'second.png'));
  f.ingestor.terminal(f.id, 'complete');
  expect(f.web.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('second.png');
  expect(f.web.read(f.a.session.sessionId)?.runStatus).toBe('complete');
  if (!finishTransfer) throw new Error('The document adapter did not request image bytes.');
  finishTransfer({ format: 'png', encoding: 'base64', data: 'png' });
  await f.delivery.idle();
  expect(f.insert.mock.calls.map(([value]) => value.layerName)).toEqual([
    'final.png',
    'second.png',
  ]);
  expect(f.web.read(f.a.session.sessionId)?.runStatus).toBe('complete');
  f.delivery.dispose();
});

test('routes preview bytes only through the interested peer capability without document image loading', () => {
  const f = fixture();
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  const run = f.runs.read(f.id)!;
  const bytes = new Blob(['preview']);
  f.delivery.preview({
    source: { run, source: f.a.sources[0]! },
    bytes,
    preview: {
      sessionId: run.session.sessionId,
      generation: run.session.generation,
      promptId: run.promptId,
      sourceId: f.a.sources[0]!.id,
      runOrder: run.order,
      sequence: 1,
      token: 'frame',
      dimensions: null,
    },
  });
  expect(f.frames).toEqual([bytes]);
  expect(f.load).not.toHaveBeenCalled();
  expect(f.insert).not.toHaveBeenCalled();
  f.delivery.dispose();
});

test('isolates a failed peer and retains shared logical image identities across recovery', async () => {
  const f = fixture();
  const deliveries: { ids: readonly string[]; filename: string | undefined }[] = [];
  const failing: WorkflowCanvasAdapter<Blob> = {
    id: 'failing-peer',
    capabilities: {
      output: {
        ordering: 'immediate',
        bind: () => ({
          receive(output) {
            deliveries.push({
              ids: output.images.map((image) => image.id),
              filename: output.state?.result?.outputs[0]?.items[0]?.file.filename,
            });
            throw new Error('Peer failed');
          },
        }),
      },
    },
  };
  f.delivery.register(failing);
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  const first = workflowFinal(f.id);
  f.ingestor.final(first);
  f.ingestor.final({ ...first, delivery: 'recovery' });
  await f.delivery.idle();
  expect(deliveries[0]?.ids).toHaveLength(1);
  expect(deliveries[0]?.filename).toBe('final.png');
  expect(deliveries[1]?.ids).toEqual([]);
  expect(f.insert).toHaveBeenCalledTimes(1);
  expect(f.web.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('final.png');
  expect(f.report).toHaveBeenCalledWith(expect.objectContaining({ message: 'Peer failed' }));
  f.delivery.dispose();
});

test('restores web state on reattachment without replaying document mutations or binding an existing run', async () => {
  const f = fixture();
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  f.ingestor.final(workflowFinal(f.id));
  await f.delivery.idle();
  const web = new WebCanvasAdapter<Blob>({ wants: () => false, receive: () => undefined });
  const unregister = f.delivery.register({ ...web.adapter, id: 'second-view' });
  expect(web.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('final.png');
  expect(f.readContext).toHaveBeenCalledTimes(1);
  await f.delivery.idle();
  expect(f.insert).toHaveBeenCalledTimes(1);
  f.results.synchronize([f.b.session]);
  f.ingestor.closeSession(f.a.session.sessionId);
  expect(web.read(f.a.session.sessionId)).toBeNull();
  expect(f.web.read(f.a.session.sessionId)).toBeNull();
  unregister();
  web.dispose();
  f.delivery.dispose();
});

test('refuses late registration to take over a run bound to another document', async () => {
  const f = fixture();
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  const bind = jest
    .fn<WorkflowCanvasAdapter<Blob>['capabilities']['output']['bind']>()
    .mockReturnValue(null);
  f.delivery.register({
    id: 'late-editor',
    capabilities: { output: { ordering: 'serial', bind } },
  });
  f.ingestor.final(workflowFinal(f.id));
  await f.delivery.idle();
  expect(bind).not.toHaveBeenCalled();
  expect(f.insert).toHaveBeenCalledTimes(1);
  f.delivery.dispose();
});

test('pins explicit destination selection at submission instead of broadcasting to every registered peer', async () => {
  const f = fixture();
  f.targets.splice(0, f.targets.length, 'web');
  const first = f.ingestor.submitting(f.a);
  f.targets.splice(0, f.targets.length, 'editor');
  f.ingestor.accepted(first, f.id);
  f.ingestor.final(workflowFinal(f.id, 'web-only.png'));
  await f.delivery.idle();
  expect(f.readContext).not.toHaveBeenCalled();
  expect(f.insert).not.toHaveBeenCalled();
  expect(f.web.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'web-only.png',
  );
  const secondId = promptId('document-run');
  f.ingestor.accepted(f.ingestor.submitting(f.a), secondId);
  f.ingestor.final(workflowFinal(secondId, 'document-only.png'));
  await f.delivery.idle();
  expect(f.insert.mock.calls.map(([value]) => value.layerName)).toEqual(['document-only.png']);
  expect(f.web.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'web-only.png',
  );
  f.delivery.dispose();
});

test('inserts each finished output into the captured document while the job is still running', async () => {
  const f = fixture();
  const submission = f.ingestor.submitting(f.a);
  expect(f.readContext).toHaveBeenCalledTimes(1);
  f.readContext.mockResolvedValue({
    ...f.context,
    document: { ...f.context.document!, id: 'other' },
  });
  f.ingestor.accepted(submission, f.id);
  f.ingestor.started(f.id);
  f.ingestor.final(workflowFinal(f.id));
  await f.delivery.idle();
  expect(f.insert).toHaveBeenCalledTimes(1);
  expect(f.results.read(f.a.session.sessionId)?.runStatus).toBe('running');
  expect(f.insert.mock.calls[0]?.[0]).toMatchObject({
    document: f.context.document,
    layerName: 'final.png',
  });
  expect(f.load.mock.calls[0]?.[0]).toEqual({
    filename: 'final.png',
    subfolder: '',
    type: 'output',
  });
  f.ingestor.final(workflowFinal(f.id, 'second.png'));
  await f.delivery.idle();
  expect(f.insert.mock.calls.map(([image]) => image.layerName)).toEqual([
    'final.png',
    'second.png',
  ]);
  expect(f.results.read(f.a.session.sessionId)?.runStatus).toBe('running');
  f.ingestor.terminal(f.id, 'complete');
  await f.delivery.idle();
  expect(f.insert).toHaveBeenCalledTimes(2);
  f.delivery.dispose();
});

test('attributes early completion, deduplicates recovery and inserts every image in a batch serially', async () => {
  const f = fixture();
  const submission = f.ingestor.submitting(f.a);
  const first = workflowFinal(f.id);
  const batch = {
    ...first,
    payload: {
      ...first.payload,
      images: [...first.payload.images, ...workflowFinal(f.id, 'second.png').payload.images],
    },
  };
  f.ingestor.final(batch);
  f.ingestor.terminal(f.id, 'complete');
  f.ingestor.accepted(submission, f.id);
  f.ingestor.final({ ...batch, delivery: 'history-copy' });
  f.ingestor.recovered(f.id, 'complete');
  await f.delivery.idle();
  expect(f.insert.mock.calls.map(([value]) => value.layerName)).toEqual([
    'final.png',
    'second.png',
  ]);
  expect(f.report).not.toHaveBeenCalled();
  f.delivery.dispose();
});

test('treats independent output sources as separate outputs even when they reference the same file', async () => {
  const f = fixture();
  const submission = {
    ...f.a,
    sources: [
      ...f.a.sources,
      { ...f.a.sources[0]!, id: outputSourceId('other-output'), executionPath: '99', order: 1 },
    ],
  };
  f.ingestor.accepted(f.ingestor.submitting(submission), f.id);
  f.ingestor.final(workflowFinal(f.id));
  await f.delivery.idle();
  expect(f.insert).toHaveBeenCalledTimes(1);
  f.ingestor.final({ ...workflowFinal(f.id), executionPaths: ['99'] });
  await f.delivery.idle();
  expect(f.insert).toHaveBeenCalledTimes(2);
  expect(f.results.read(f.a.session.sessionId)?.outputs).toHaveLength(2);
  f.delivery.dispose();
});

test('keeps overlapping runs independent even when an older result is no longer the displayed result', async () => {
  const f = fixture();
  const first = f.ingestor.submitting(f.a);
  const second = f.ingestor.submitting(f.a);
  const newer = promptId('newer');
  f.ingestor.accepted(second, newer);
  f.ingestor.accepted(first, f.id);
  f.ingestor.final(workflowFinal(newer, 'newer.png'));
  f.ingestor.terminal(newer, 'complete');
  f.ingestor.final(workflowFinal(f.id, 'older.png'));
  f.ingestor.terminal(f.id, 'complete');
  await f.delivery.idle();
  expect(f.insert.mock.calls.map(([value]) => value.layerName)).toEqual(['newer.png', 'older.png']);
  f.delivery.dispose();
});

test.each(['cancelled', 'error'] as const)(
  'keeps finished output available when later work is %s',
  async (status) => {
    const f = fixture();
    f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
    f.ingestor.final(workflowFinal(f.id));
    f.ingestor.terminal(f.id, status);
    await f.delivery.idle();
    expect(f.insert.mock.calls.map(([image]) => image.layerName)).toEqual(['final.png']);
    f.ingestor.final(workflowFinal(f.id, 'late.png'));
    await f.delivery.idle();
    expect(f.insert).toHaveBeenCalledTimes(1);
    f.ingestor.dispose();
    f.delivery.dispose();
  },
);

test('delivers incremental and replacement image notifications once without duplicating recovered outputs', async () => {
  const f = fixture();
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  const first = workflowFinal(f.id);
  f.ingestor.final(first);
  await f.delivery.idle();
  expect(f.insert).toHaveBeenCalledTimes(1);
  const second = workflowFinal(f.id, 'second.png');
  f.ingestor.final({ ...second, merge: true });
  await f.delivery.idle();
  expect(f.insert.mock.calls.map(([image]) => image.layerName)).toEqual([
    'final.png',
    'second.png',
  ]);
  f.ingestor.final({ ...second, merge: true });
  f.ingestor.final({
    ...first,
    payload: { ...first.payload, images: [...first.payload.images, ...second.payload.images] },
    delivery: 'history-copy',
  });
  f.ingestor.terminal(f.id, 'complete');
  f.ingestor.recovered(f.id, 'complete');
  await f.delivery.idle();
  expect(f.insert).toHaveBeenCalledTimes(2);
  f.delivery.dispose();
});

test('delivers recovered output for an observed live run but never replays restored or foreign results', async () => {
  const f = fixture();
  f.ingestor.final(workflowFinal(promptId('foreign')));
  f.results.restore(f.a.session, f.a.sources[0]!, workflowFinal(f.id).payload, 'saved-workflow');
  await f.delivery.idle();
  expect(f.insert).not.toHaveBeenCalled();
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  f.ingestor.final(workflowFinal(f.id, 'recovered.png'), 1);
  f.ingestor.recovered(f.id, 'complete');
  await f.delivery.idle();
  expect(f.insert.mock.calls.map(([value]) => value.layerName)).toEqual(['recovered.png']);
  f.delivery.dispose();
  f.ingestor.dispose();
});

test('ignores input previews and transient sources while admitting final generated images', async () => {
  const f = fixture();
  const submission = {
    ...f.a,
    sources: [
      ...f.a.sources,
      { ...f.a.sources[0]!, executionPath: 'transient', transientOnly: true as const },
    ],
  };
  f.ingestor.accepted(f.ingestor.submitting(submission), f.id);
  const final = workflowFinal(f.id);
  f.ingestor.final({
    ...final,
    payload: {
      ...final.payload,
      images: final.payload.images.map((item) =>
        item ? { ...item, file: { ...item.file, type: 'input' } } : null,
      ),
    },
  });
  f.ingestor.final({ ...final, executionPaths: ['transient'] });
  f.ingestor.terminal(f.id, 'complete');
  await f.delivery.idle();
  expect(f.insert).not.toHaveBeenCalled();
  f.delivery.dispose();
});

test('cancels queued delivery when the workflow closes and when the integration is disposed', async () => {
  const f = fixture();
  const pending: { resolve?: (context: CanvasContext) => void } = {};
  f.readContext.mockImplementation(
    () =>
      new Promise((done) => {
        pending.resolve = done;
      }),
  );
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  f.ingestor.final(workflowFinal(f.id));
  f.ingestor.terminal(f.id, 'complete');
  f.ingestor.closeSession(f.a.session.sessionId);
  f.delivery.dispose();
  if (!pending.resolve) throw new Error('Context was not requested');
  pending.resolve(f.context);
  await f.delivery.idle();
  expect(f.insert).not.toHaveBeenCalled();
});

test('reports unavailable documents and native refusal without retrying or redirecting to the active document', async () => {
  const f = fixture();
  f.insert.mockResolvedValue({ status: 'stale', message: 'The destination was resized.' });
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  f.ingestor.final(workflowFinal(f.id));
  f.ingestor.terminal(f.id, 'complete');
  await f.delivery.idle();
  expect(f.report).toHaveBeenCalledWith(
    expect.objectContaining({ message: 'The destination was resized.' }),
  );
  f.ingestor.final({ ...workflowFinal(f.id), delivery: 'repeated-history' });
  await f.delivery.idle();
  expect(f.insert).toHaveBeenCalledTimes(1);
  f.readContext.mockResolvedValue({ ...f.context, document: null });
  const next = promptId('no-document');
  f.ingestor.accepted(f.ingestor.submitting(f.a), next);
  f.ingestor.final(workflowFinal(next));
  f.ingestor.terminal(next, 'complete');
  await f.delivery.idle();
  expect(f.insert).toHaveBeenCalledTimes(1);
  expect(f.report).toHaveBeenCalledWith(
    expect.objectContaining({ message: expect.stringContaining('Open a document') }),
  );
  f.delivery.dispose();
});

test('leaves display-only canvases with their existing capabilities', async () => {
  const f = fixture();
  f.canvas.readCanvas = () => ({
    ...f.context.canvas,
    capabilities: { displayOutput: true, insertImageLayer: false },
  });
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  f.ingestor.final(workflowFinal(f.id));
  f.ingestor.terminal(f.id, 'complete');
  await f.delivery.idle();
  expect(f.readContext).not.toHaveBeenCalled();
  expect(f.report).not.toHaveBeenCalled();
  expect(f.insert).not.toHaveBeenCalled();
  f.delivery.dispose();
});

test('cancels transfer at workflow retirement while preserving another job delivery', async () => {
  const f = fixture();
  const pending: { release?: () => void } = {};
  f.load.mockImplementationOnce(async (_file, cancellation) => {
    await new Promise<void>((resolve) => {
      pending.release = resolve;
    });
    expect(cancellation.cancelled()).toBe(true);
    return { format: 'png', encoding: 'base64', data: 'png' };
  });
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  f.ingestor.final(workflowFinal(f.id));
  f.ingestor.terminal(f.id, 'complete');
  for (let i = 0; i < 8; i++) await Promise.resolve();
  expect(pending.release).toBeDefined();
  f.ingestor.closeSession(f.a.session.sessionId);
  const other = promptId('other-workflow');
  f.ingestor.accepted(f.ingestor.submitting(f.b), other);
  f.ingestor.final(workflowFinal(other, 'other.png'));
  f.ingestor.terminal(other, 'complete');
  pending.release?.();
  await f.delivery.idle();
  expect(f.insert.mock.calls.map(([image]) => image.layerName)).toEqual(['other.png']);
  f.delivery.dispose();
});

test('rejects queue failures without retaining a destination or admitting a late acceptance', async () => {
  const f = fixture();
  const submission = f.ingestor.submitting(f.a);
  f.ingestor.rejected(submission);
  f.ingestor.accepted(submission, f.id);
  f.ingestor.final(workflowFinal(f.id));
  f.ingestor.terminal(f.id, 'complete');
  await f.delivery.idle();
  expect(f.insert).not.toHaveBeenCalled();
  f.delivery.dispose();
});
