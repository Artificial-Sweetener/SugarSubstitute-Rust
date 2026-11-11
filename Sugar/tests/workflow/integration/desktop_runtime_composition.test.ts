/** @jest-environment node */
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
/** Verify committed startup, accepted-before-final durability and restore-only scene/view composition. */
import { desktopRuntimeFixture } from '../support/desktop_runtime.js';
import { DesktopSessionCancellation } from '../../../frontend/comfyui/ui/workflow/domain/DesktopSession.js';
import { DesktopSessionClient } from '../../../frontend/comfyui/ui/workflow/application/DesktopSessionClient.js';
import { desktopSessionChunks } from '../../../frontend/comfyui/ui/workflow/application/DesktopSessionTransfer.js';
import { createHash } from 'node:crypto';
import type {
  SessionMessage,
  SessionOperation,
  SessionFence,
  SessionCheckpoint,
} from '../../../integrations/contracts/SessionProtocol.generated.js';
import { utf8Bytes } from '../../../integrations/shared/SessionPrimitives.js';
import { isDesktopCheckpoint } from '../../../integrations/shared/SessionSnapshot.js';
import { outputSourceId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { canvasInspectionSlot } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionSlots.js';

test('fresh temp scene finals request one verified asset while saved PNG and animated WebP keep their backend originals', async () => {
  const f = await desktopRuntimeFixture();
  try {
    const history = new Map<string, object>();
    const requestedHistory: string[] = [];
    Object.assign(f.api, {
      fetchApi: async (path: string, options: { readonly signal: AbortSignal }) => {
        if (options.signal.aborted) throw new Error('History request was cancelled.');
        const completed = history.get(path);
        if (!completed) throw new Error(`Unexpected exact-job history request: ${path}`);
        requestedHistory.push(path);
        return { ok: true, json: async () => completed };
      },
    });
    const text = '**Morning\nportrait-a\n\n**Night\nportrait-b';
    const graph = {
      nodes: [
        {
          id: 1,
          type: 'PrimitiveStringMultiline',
          widgets: [{ name: 'value', type: 'customtext', value: text }],
        },
        { id: 4, type: 'ImageScale', widgets_values: ['bilinear', 512, 384, 'disabled'] },
        { id: 7, type: 'PreviewImage', title: 'Preview' },
        { id: 8, type: 'SaveImage', title: 'Saved' },
        { id: 9, type: 'SaveAnimatedWEBP', title: 'Animated' },
      ],
      extra: {},
    };
    const workflow = f.host.workflow.activeWorkflow;
    if (!workflow) throw new Error('Missing current workflow.');
    Object.assign(f.host.app, { graph });
    await f.host.app.loadGraphData(graph, true, true, workflow);
    f.host.committed();
    f.setReceipt(async (file) => ({
      available: true,
      receipt: {
        file: {
          storage: file.type,
          filename: file.filename,
          subfolder: file.subfolder,
          managedArtifact: null,
        },
        digest: (file.type === 'temp' ? 'a' : file.filename.endsWith('.webp') ? 'c' : 'b').repeat(
          64,
        ),
        bytes: file.type === 'temp' ? 128 : 256,
        width: 512,
        height: 512,
        media: 'image',
        mime: file.filename.endsWith('.webp') ? 'image/webp' : 'image/png',
        metadataEmbedded: false,
        animated: file.filename.endsWith('.webp'),
      },
    }));
    const payload = await f.host.app.graphToPrompt();
    Object.assign(payload.output, {
      '1': { class_type: 'PrimitiveStringMultiline', inputs: { value: text } },
      '7': { class_type: 'PreviewImage', inputs: {} },
      '8': { class_type: 'SaveImage', inputs: {} },
      '9': { class_type: 'SaveAnimatedWEBP', inputs: {} },
    });
    const scenes = f.execution.scenes.expand(payload);
    expect(scenes).toHaveLength(2);
    if (!scenes) throw new Error('Missing actual scene expansion.');
    for (const [index, scene] of scenes.entries()) {
      const prompt = `temp-scene-${index}`;
      const observed = f.execution.submissionObserver.capture(scene);
      if (!observed) throw new Error('Missing scene submission ownership.');
      observed.prepared(scene);
      observed.accepted({ prompt_id: prompt });
      const outputs = {
        '7': {
          images: [
            { filename: `preview-${index}-1.png`, subfolder: '', type: 'temp' },
            { filename: `preview-${index}-2.png`, subfolder: '', type: 'temp' },
          ],
        },
        '8': { images: [{ filename: `saved-${index}.png`, subfolder: '', type: 'output' }] },
        '9': {
          images: [{ filename: `animated-${index}.webp`, subfolder: '', type: 'output' }],
          animated: [true],
          frame_count: 0,
        },
      };
      history.set(`/history/${prompt}`, {
        [prompt]: { outputs, status: { completed: true, status_str: 'success' } },
      });
      for (const [node, output] of Object.entries(outputs))
        f.api.emit('executed', { prompt_id: prompt, node, output });
      f.api.emit('execution_success', { prompt_id: prompt });
    }
    const checkpoint = await f.runtime.capture(f.nextId(), 140, new DesktopSessionCancellation());
    expect(isDesktopCheckpoint(checkpoint)).toBe(true);
    const entry = checkpoint.entries[0];
    expect(entry?.submissions.map((item) => item.scene?.title)).toEqual(['Morning', 'Night']);
    expect(entry?.outputs).toHaveLength(8);
    expect(entry?.outputs.filter((item) => item.file.storage === 'temp')).toHaveLength(4);
    expect(
      entry?.outputs
        .filter((item) => item.file.storage === 'temp')
        .every((item) => item.assetDigest === item.digest),
    ).toBe(true);
    expect(
      entry?.outputs
        .filter((item) => item.file.storage === 'output')
        .every((item) => item.assetDigest === null),
    ).toBe(true);
    expect(entry?.outputs.filter((item) => item.mime === 'image/webp')).toHaveLength(2);
    expect(checkpoint.assets).toEqual([
      { version: 1, digest: 'a'.repeat(64), bytes: 128, extension: 'png', media: 'image' },
    ]);
    expect(requestedHistory).toEqual(['/history/temp-scene-0', '/history/temp-scene-1']);
    if (!entry) throw new Error('Missing captured scene entry.');
    const retained = entry.outputs.find((output) => output.file.filename === 'preview-1-2.png');
    const acceptedSource = entry.sources.find((source) => source.id === retained?.sourceId);
    if (!retained || !retained.scene || !acceptedSource)
      throw new Error('Missing accepted Night batch2 final.');
    f.host.setGraph({
      ...graph,
      nodes: graph.nodes.map((node) =>
        node.id === 4 ? { ...node, widgets_values: ['bilinear', 513, 384, 'disabled'] } : node,
      ),
    });
    workflow.isModified = true;
    f.host.app.rootGraph.onAfterChange(f.host.app.rootGraph);
    await Promise.resolve();
    const context = f.execution.sessions.read().active;
    const view = context ? f.coordinator.primary(context.sessionId) : null;
    if (!context || !view) throw new Error('Missing current canvas occurrence.');
    let catalog = f.coordinator.catalog(view);
    if (!catalog) throw new Error('Missing current canvas catalog.');
    f.selection.apply(view, catalog, {
      type: 'select',
      selection: { sourceId: outputSourceId('4'), batchIndex: 0 },
    });
    const fallback = await f.runtime.capture(f.nextId(), 141, new DesktopSessionCancellation());
    const fallbackEntry = fallback.entries[0];
    const graphSource = fallbackEntry?.sources.find(
      (source) =>
        source.nodeId === '4' && source.graphRevision === fallbackEntry.document.graphRevision,
    );
    expect(fallbackEntry?.document.dirty).toBe(true);
    expect(fallbackEntry?.document.graphRevision).toBeGreaterThan(acceptedSource.graphRevision);
    expect(fallbackEntry?.document.graphJson).toContain('513');
    expect(fallbackEntry?.document.baselineGraphJson).toContain('512');
    expect(fallbackEntry?.view.selection).toBeNull();
    expect(fallbackEntry?.view.navigation.sourceId).toBe(graphSource?.id);
    expect(isDesktopCheckpoint(fallback)).toBe(true);
    catalog = f.coordinator.catalog(view);
    const night = catalog?.result.scenes?.scenes.find((scene) => scene.title === 'Night');
    if (!catalog || !night) throw new Error('Missing retained Night scene catalog.');
    f.selection.apply(view, catalog, { type: 'select-scene', scene: night.key });
    catalog = f.coordinator.catalog(view);
    if (!catalog) throw new Error('Missing selected Night catalog.');
    const selection = { sourceId: outputSourceId('7'), batchIndex: 1 };
    f.selection.apply(view, catalog, { type: 'select', selection });
    f.selection.apply(view, catalog, { type: 'link', enabled: false });
    const item = f.selection.items(view, catalog).primary;
    if (!item) throw new Error('Missing selected retained batch2 image.');
    expect(item.file.filename).toBe(retained.file.filename);
    f.inspections.setLinked(view, false, null);
    const inspection = {
      mode: 'custom' as const,
      region: { centerX: -2.25, centerY: 1.75, spanX: 0.002, spanY: 0.003 },
    };
    f.inspections.record(view, canvasInspectionSlot(selection, item, catalog), inspection);
    const edited = await f.runtime.capture(f.nextId(), 142, new DesktopSessionCancellation());
    const editedEntry = edited.entries[0];
    expect(isDesktopCheckpoint(edited)).toBe(true);
    expect(editedEntry?.outputs.find((output) => output.id === retained.id)).toEqual(retained);
    expect(editedEntry?.view.selection).toMatchObject({
      outputId: retained.id,
      sourceId: retained.sourceId,
      sceneId: retained.scene.sceneId,
      batchIndex: 1,
    });
    expect(editedEntry?.view.navigation).toMatchObject({
      sceneId: retained.scene.sceneId,
      sourceId: retained.sourceId,
      batchIndex: 1,
    });
    expect(editedEntry?.view.cameras).toContainEqual({
      outputId: retained.id,
      inspection: { mode: inspection.mode, ...inspection.region },
    });
    expect(f.errors).toEqual([]);
  } finally {
    f.dispose();
  }
});

test.each([
  ['unsupported MIME', 'unavailable'],
  ['conflicting digest metadata', 'identity-mismatch'],
] as const)(
  'fresh temporary finals refuse %s without replacing the previous checkpoint',
  async (failure, code) => {
    const f = await desktopRuntimeFixture();
    try {
      const workflow = f.host.workflow.activeWorkflow;
      if (!workflow) throw new Error('Missing current workflow.');
      await f.host.app.loadGraphData(
        { nodes: [{ id: 7, type: 'PreviewImage', title: 'Preview' }], extra: {} },
        true,
        true,
        workflow,
      );
      f.host.committed();
      const previous = await f.runtime.capture(f.nextId(), 150, new DesktopSessionCancellation());
      f.runtime.acknowledged(previous.id, previous.revision);
      f.setReceipt(async (file) => ({
        available: true,
        receipt: {
          file: {
            storage: file.type,
            filename: file.filename,
            subfolder: file.subfolder,
            managedArtifact: null,
          },
          digest: 'a'.repeat(64),
          bytes:
            failure === 'conflicting digest metadata' && file.filename === 'second.png' ? 129 : 128,
          width: 512,
          height: 512,
          media: 'image',
          mime: failure === 'unsupported MIME' ? 'image/tiff' : 'image/png',
          metadataEmbedded: false,
          animated: false,
        },
      }));
      const observed = f.execution.submissionObserver.capture(await f.host.app.graphToPrompt());
      if (!observed) throw new Error('Missing actual queue observation.');
      observed.accepted({ prompt_id: 'temporary-refusal' });
      f.api.emit('executed', {
        prompt_id: 'temporary-refusal',
        node: '7',
        output: {
          images: [
            { filename: 'first.png', type: 'temp', subfolder: '' },
            { filename: 'second.png', type: 'temp', subfolder: '' },
          ],
        },
      });
      await expect(
        f.runtime.capture(f.nextId(), 151, new DesktopSessionCancellation()),
      ).rejects.toMatchObject({ code });
      f.setReceipt(async (file) => ({
        available: true,
        receipt: {
          file: {
            storage: file.type,
            filename: file.filename,
            subfolder: file.subfolder,
            managedArtifact: null,
          },
          digest: 'a'.repeat(64),
          bytes: 128,
          width: 512,
          height: 512,
          media: 'image',
          mime: 'image/png',
          metadataEmbedded: false,
          animated: false,
        },
      }));
      const recovered = await f.runtime.capture(f.nextId(), 152, new DesktopSessionCancellation());
      expect(isDesktopCheckpoint(recovered)).toBe(true);
      expect(recovered.previousId).toBe(previous.id);
      expect(recovered.entries[0]?.outputs).toHaveLength(2);
      expect(recovered.assets).toHaveLength(1);
      expect(f.errors).toEqual([]);
    } finally {
      f.dispose();
    }
  },
);

test('production runtime installs before startup commitment and retains accepted ownership before any final exists', async () => {
  const f = await desktopRuntimeFixture();
  expect(f.runtime.readiness().committed).toBe(false);
  await expect(f.runtime.capture(f.nextId(), 10, new DesktopSessionCancellation())).rejects.toThrow(
    'committed',
  );
  f.host.committed();
  expect(f.runtime.readiness().committed).toBe(true);
  const payload = await f.host.app.graphToPrompt();
  const observation = f.execution.submissionObserver.capture(payload);
  if (!observation) throw new Error('Missing actual queue observation.');
  const before = f.runtime.fence().bindings[0];
  f.host.setGraph({
    nodes: [{ id: 7, type: 'SaveImage', title: 'Edited after construction' }],
    extra: {},
  });
  f.host.app.rootGraph.onAfterChange(f.host.app.rootGraph);
  await Promise.resolve();
  observation.accepted({ prompt_id: 'accepted-before-final' });
  const captured = await f.runtime.capture(f.nextId(), 44, new DesktopSessionCancellation());
  expect(captured.revision).toBe(44);
  expect(captured.entries[0]?.outputs).toEqual([]);
  expect(captured.entries[0]?.submissions[0]).toMatchObject({
    promptId: 'accepted-before-final',
    graphRevision: before?.graphRevision,
  });
  expect(captured.entries[0]?.submissions[0]?.sources[0]?.label).toBe('Final');
  expect(captured.entries[0]?.document.graphRevision).toBeGreaterThan(before?.graphRevision ?? -1);
  f.api.emit('executed', {
    prompt_id: 'accepted-before-final',
    node: '7',
    output: { images: [{ filename: 'final.png' }] },
  });
  const final = await f.runtime.capture(f.nextId(), 45, new DesktopSessionCancellation());
  expect(final.entries[0]?.outputs[0]?.sourceId).toBe(
    final.entries[0]?.submissions[0]?.sources[0]?.id,
  );
  expect(final.entries[0]?.outputs[0]?.runId).toBe(final.entries[0]?.submissions[0]?.runId);
  expect(final.previousId).toBeNull();
  f.runtime.acknowledged(final.id, final.revision);
  expect(
    (await f.runtime.capture(f.nextId(), 46, new DesktopSessionCancellation())).previousId,
  ).toBe(final.id);
  f.dispose();
  expect(f.errors).toEqual([]);
});

test('real G0 transport handshake binds the tabs Comfy reopened to results and cameras without generation replay', async () => {
  const f = await desktopRuntimeFixture();
  // Comfy reopens the tabs from its own persistence before startup commits.
  const tabs = await f.host.reopen(
    f.checkpoint.entries.map((entry) => entry.document),
    f.checkpoint.entries.findIndex((entry) => entry.id === f.checkpoint.activeEntry),
  );
  f.host.committed();
  const before = f.execution.sessions.read().active?.sessionId;
  const generation: string[] = [];
  const stopGeneration = f.execution.subscribeGeneration((event) => generation.push(event.type));
  const sent: SessionMessage[] = [];
  const admitted: SessionFence[] = [];
  f.setResolve(async (output, binding) => {
    expect(admitted.at(-1)?.bindings).toContainEqual(binding);
    return {
      available: true,
      receipt: {
        file: output.file,
        digest: output.digest,
        bytes: output.bytes,
        width: output.width,
        height: output.height,
        media: output.media,
        mime: output.mime,
        metadataEmbedded: output.metadataEmbedded,
        animated: false,
      },
    };
  });
  const digest = async (text: string) => createHash('sha256').update(text, 'utf8').digest('hex');
  const client = new DesktopSessionClient({
    transport: {
      scope: f.checkpoint.scope,
      origin: f.origin,
      webview: f.fence.webview,
      send: async (message) => {
        await Promise.resolve();
        if (message.operation.kind === 'ready') admitted.push(message.operation.fence);
        sent.push(message);
      },
      subscribe: () => () => {},
    },
    runtime: f.runtime,
    nextId: f.nextId,
    digest,
    report: (error) => f.errors.push(error),
  });
  client.install();
  const requestId = f.nextId();
  const message = (operation: SessionOperation): SessionMessage => ({
    protocol: 'sugarcubes.desktop-session',
    version: 1,
    requestId,
    scope: f.checkpoint.scope,
    origin: f.origin,
    operation,
  });
  const text = JSON.stringify(f.checkpoint);
  const chunks = desktopSessionChunks(text);
  const transferId = f.nextId();
  await client.receive(
    message({
      kind: 'begin',
      transfer: {
        id: transferId,
        checkpointId: f.checkpoint.id,
        revision: f.checkpoint.revision,
        bytes: utf8Bytes(text),
        chunks: chunks.length,
        digest: await digest(text),
        direction: 'materialize',
      },
    }),
  );
  for (const [index, text] of chunks.entries())
    await client.receive(message({ kind: 'chunk', transferId, index, text }));
  await client.receive(message({ kind: 'commit', transferId }));
  await client.receive(message({ kind: 'materialize', transferId, fence: f.runtime.fence() }));
  const acknowledged = sent.find(
    (message) =>
      message.operation.kind === 'acknowledged' && message.operation.phase === 'materialized',
  );
  expect(f.errors).toEqual([]);
  expect(acknowledged?.operation).toMatchObject({
    checkpointId: f.checkpoint.id,
    revision: f.checkpoint.revision,
    phase: 'materialized',
  });
  expect(f.execution.sessions.read().active?.sessionId).toBe(before);
  expect(f.host.workflow.openWorkflows).toEqual(tabs);
  expect(f.runtime.fence().bindings.map((binding) => binding.entryId)).toEqual(
    f.checkpoint.entries.map((entry) => entry.id),
  );
  expect(f.execution.outstandingRuns()).toEqual([]);
  expect(generation).not.toContain('accepted');
  expect(generation).not.toContain('output');
  expect(generation).not.toContain('submitted');
  const captured = await f.runtime.capture(f.nextId(), 88, new DesktopSessionCancellation());
  expect(captured.entries[0]?.outputs).toEqual(f.checkpoint.entries[0]?.outputs);
  expect(captured.assets).toEqual(f.checkpoint.assets);
  expect(captured.entries[0]?.input).toEqual(f.checkpoint.entries[0]?.input);
  expect(captured.entries[0]?.view.sharedInspection).toEqual(
    f.checkpoint.entries[0]?.view.sharedInspection,
  );
  expect(captured.entries[0]?.view.cameras).toEqual(f.checkpoint.entries[0]?.view.cameras);
  expect(captured.entries[0]?.view.follow).toBe('manual');
  expect(captured.entries[0]?.view.sourcePreview).toBe(false);
  client.dispose();
  stopGeneration();
  f.dispose();
  expect(f.errors).toEqual([]);
});

test('a checkpoint naming fewer tabs than Comfy reopened leaves every reopened tab open and binds only its own', async () => {
  const f = await desktopRuntimeFixture();
  const kept = f.checkpoint.entries[0];
  if (!kept || f.checkpoint.entries.length < 2)
    throw new Error('The fixture needs several entries.');
  const digests = new Set(kept.outputs.map((output) => output.assetDigest));
  const stale: SessionCheckpoint = {
    ...f.checkpoint,
    entries: [kept],
    activeEntry: kept.id,
    assets: f.checkpoint.assets.filter((asset) => digests.has(asset.digest)),
  };
  const tabs = await f.host.reopen(
    f.checkpoint.entries.map((entry) => entry.document),
    0,
  );
  f.host.committed();
  f.setResolve(async (output) => ({
    available: true,
    receipt: {
      file: output.file,
      digest: output.digest,
      bytes: output.bytes,
      width: output.width,
      height: output.height,
      media: output.media,
      mime: output.mime,
      metadataEmbedded: output.metadataEmbedded,
      animated: false,
    },
  }));
  const bindings = await f.runtime.materialize(stale, new DesktopSessionCancellation());
  expect(f.host.workflow.openWorkflows).toEqual(tabs);
  expect(bindings.map((binding) => binding.entryId)).toEqual([kept.id]);
  expect(f.errors).toEqual([]);
  f.dispose();
});

test('a competing committed load cancels delayed artifact restoration before pixels or view state can be published', async () => {
  const f = await desktopRuntimeFixture();
  await f.host.reopen(
    f.checkpoint.entries.map((entry) => entry.document),
    f.checkpoint.entries.findIndex((entry) => entry.id === f.checkpoint.activeEntry),
  );
  f.host.committed();
  let release: () => void = () => {};
  let entered: () => void = () => {};
  const waiting = new Promise<void>((resolve) => {
    entered = resolve;
  });
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  f.setResolve(async (output) => {
    entered();
    await gate;
    return {
      available: true,
      receipt: {
        file: output.file,
        digest: output.digest,
        bytes: output.bytes,
        width: output.width,
        height: output.height,
        media: output.media,
        mime: output.mime,
        metadataEmbedded: output.metadataEmbedded,
        animated: false,
      },
    };
  });
  const cancellation = new DesktopSessionCancellation();
  const pending = f.runtime.materialize(f.checkpoint, cancellation);
  await waiting;
  const current = f.host.workflow.activeWorkflow;
  if (!current) throw new Error('Missing materialized workflow.');
  await f.host.app.loadGraphData(
    { nodes: [], extra: { user: 'replacement' } },
    true,
    true,
    current,
  );
  expect(cancellation.cancelled()).toBe(true);
  release();
  await expect(pending).rejects.toMatchObject({ code: 'cancelled' });
  expect(f.execution.results.snapshots().flatMap((result) => result.outputs)).toEqual([]);
  await expect(
    f.runtime.materialize(f.checkpoint, new DesktopSessionCancellation()),
  ).rejects.toMatchObject({ code: 'stale' });
  f.dispose();
});

test.each(['graph', 'workflow', 'generation'] as const)(
  'explicit %s intent before startup materialization keeps new work capturable and refuses replay over it',
  async (intent) => {
    const f = await desktopRuntimeFixture();
    f.host.committed();
    const original = f.host.workflow.activeWorkflow;
    if (!original) throw new Error('Missing initial workflow.');
    if (intent === 'graph') {
      f.host.setGraph({ nodes: [{ id: 7, type: 'SaveImage', title: 'Authored after startup' }] });
      f.host.app.rootGraph.onAfterChange(f.host.app.rootGraph);
      await Promise.resolve();
    } else if (intent === 'workflow') {
      await f.host.app.loadGraphData(
        { nodes: [{ id: 7, type: 'SaveImage', title: 'User loaded workflow' }] },
        true,
        true,
        original,
      );
    } else {
      const observation = f.execution.submissionObserver.capture(await f.host.app.graphToPrompt());
      if (!observation) throw new Error('Missing queue observation.');
      observation.accepted({ prompt_id: 'new-user-generation' });
    }
    await expect(
      f.runtime.materialize(f.checkpoint, new DesktopSessionCancellation()),
    ).rejects.toMatchObject({ code: 'stale' });
    expect(f.host.workflow.activeWorkflow).toBe(original);
    expect(f.runtime.readiness().committed).toBe(true);
    const fresh = await f.runtime.capture(f.nextId(), 120, new DesktopSessionCancellation());
    expect(fresh.entries[0]?.document.name).toBe('Initial');
    expect(fresh.previousId).toBeNull();
    if (intent === 'generation')
      expect(fresh.entries[0]?.submissions[0]?.promptId).toBe('new-user-generation');
    f.dispose();
    expect(f.errors).toEqual([]);
  },
);

test('Comfy graph pan and scale are navigation observations with their own revision, independent of authored graph identity', async () => {
  const f = await desktopRuntimeFixture();
  f.host.committed();
  const before = f.runtime.fence().bindings[0];
  if (!before) throw new Error('Missing current occurrence binding.');
  f.host.app.canvas.ds.offset = [-23, 51];
  f.host.app.canvas.ds.scale = 2.5;
  const captured = await f.runtime.capture(f.nextId(), 130, new DesktopSessionCancellation());
  const after = f.runtime.fence().bindings[0];
  expect(after?.graphRevision).toBe(before.graphRevision);
  expect(after?.navigationRevision).toBeGreaterThan(before.navigationRevision);
  expect(captured.entries[0]?.document.view).toEqual({ x: -23, y: 51, scale: 2.5 });
  expect(captured.entries[0]?.navigationRevision).toBe(after?.navigationRevision);
  f.dispose();
  expect(f.errors).toEqual([]);
});
