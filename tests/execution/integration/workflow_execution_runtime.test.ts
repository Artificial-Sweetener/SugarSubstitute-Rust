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
/** Verify unified runtime composition against public host load, queue and event boundaries. */

import { ComfyWorkflowExecutionRuntime } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowExecutionRuntime.js';
import { ComfyPromptQueueBridge } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptQueueBridge.js';
import { workflowFixture, WorkflowHostFixture } from '../support/workflow_host.js';
import { WorkflowEventApi, WorkflowTestClock } from '../support/workflow_results.js';
import { buildCubeOutputExecutionId } from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';
import { WorkflowHistoryApi, settleWorkflowHistory } from '../support/workflow_history.js';

/** Capture a host prompt at entry, independently of later graph or tab changes. */
class ExecutionHost extends WorkflowHostFixture {
  nodeOutputs: Record<string, unknown> = {};
  nodePreviewImages: Record<string, unknown> = {};
  pauseConstruction: (() => Promise<void>) | null = null;
  /** Preserve the same async public construction boundary used by real Comfy queueing. */
  async graphToPrompt(): Promise<object> {
    const workflow = this.extensionManager.workflow.activeWorkflow?.changeTracker.activeState;
    const payload = {
      workflow,
      output: {
        '7': { class_type: 'PreviewImage', _meta: { title: 'Image' } },
        '42:23': { class_type: 'PreviewImage', _meta: { title: 'Detail' } },
      },
    };
    if (this.pauseConstruction) await this.pauseConstruction();
    return payload;
  }
}

/** Expose transport independently of event subscriptions so recovery requests remain observable. */
class ExecutionApi extends WorkflowEventApi {
  readonly history = new WorkflowHistoryApi();
  #sequence = 0;
  pauseTransport: (() => Promise<void>) | null = null;
  /** Return unique backend job identities while retaining the public receiver contract. */
  async queuePrompt(_position: number, _payload: unknown): Promise<unknown> {
    const id = `job-${++this.#sequence}`;
    if (this.pauseTransport) await this.pauseTransport();
    return { prompt_id: id };
  }
  /** Forward exact history requests into manually controlled external transport. */
  fetchApi(path: string, options: { signal: AbortSignal }): Promise<unknown> {
    return this.history.fetchApi(path, options);
  }
}

/** Compose real owners around two host objects with colliding serialized and numeric IDs. */
function fixture() {
  const a = workflowFixture('same');
  const b = workflowFixture('same');
  const host = new ExecutionHost(a);
  host.extensionManager.workflow.openWorkflows.push(b);
  const api = new ExecutionApi();
  const errors: unknown[] = [];
  let key = 0;
  const created: Blob[] = [];
  const revoked: string[] = [];
  const visibility = { visible: true, listeners: new Set<() => void>() };
  const clock = new WorkflowTestClock();
  const runtime = new ComfyWorkflowExecutionRuntime({
    app: host,
    api,
    logger: {
      error: (...args: unknown[]) => {
        errors.push(args);
      },
      warn: () => undefined,
    },
    nextKey: () => String(++key),
    clock,
    previewVisibility: {
      read: () => visibility.visible,
      subscribe(listener) {
        visibility.listeners.add(listener);
        return () => {
          visibility.listeners.delete(listener);
        };
      },
    },
    previewUrls: {
      create(blob) {
        created.push(blob);
        return `blob:owned-${created.length}`;
      },
      revoke(url) {
        revoked.push(url);
      },
    },
  });
  const bridge = new ComfyPromptQueueBridge({
    api,
    prepare: (payload) => {
      runtime.assertCurrentPrompt(payload);
      return payload;
    },
    transform: (payload) => Promise.resolve(payload),
    observer: runtime.submissionObserver,
  });
  return { a, b, host, api, runtime, bridge, errors, created, revoked, clock, visibility };
}

test('stores background workflow finals while current cube/thumbnail projections follow the committed tab', async () => {
  const f = fixture();
  f.runtime.install();
  f.bridge.install();
  const a = f.runtime.sessions.read().active;
  if (!a) throw new Error('Missing workflow A');
  await f.api.queuePrompt(0, await f.host.graphToPrompt());
  await f.host.loadGraphData({}, true, true, f.b);
  const b = f.runtime.sessions.read().active;
  if (!b) throw new Error('Missing workflow B');
  await f.api.queuePrompt(0, await f.host.graphToPrompt());
  f.api.emit('executed', {
    prompt_id: 'job-1',
    node: '42:23',
    output: { images: [{ filename: 'a.png' }] },
  });
  expect(f.runtime.outputs.output('42:23')).toBeNull();
  f.api.emit('executed', {
    prompt_id: 'job-2',
    node: '42:23',
    output: { images: [{ filename: 'b.png' }] },
  });
  expect(f.runtime.outputs.output('42:23')?.items[0]?.file.filename).toBe('b.png');
  expect(f.runtime.results.read(a.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('a.png');
  await f.host.loadGraphData({}, true, true, f.a);
  expect(f.runtime.outputs.output('42:23')?.items[0]?.file.filename).toBe('a.png');
  f.bridge.dispose();
  f.runtime.dispose();
  expect(f.host.subscriberCount).toBe(0);
  expect(f.errors).toEqual([]);
});

test('restores only the named workflow snapshot before publishing its session, filtering orphaned source keys', () => {
  const f = fixture();
  f.a.changeTracker.activeState = { nodes: [{ id: 7, title: 'Image', type: 'PreviewImage' }] };
  f.a.changeTracker.nodeOutputs = {
    '7': { images: [{ filename: 'owned.png' }] },
    '999': { images: [{ filename: 'orphan.png' }] },
  };
  f.host.nodeOutputs = { '7': { images: [{ filename: 'global-foreign.png' }] } };
  f.runtime.install();
  expect(f.runtime.outputs.output('7')?.items[0]?.file.filename).toBe('owned.png');
  expect(f.runtime.outputs.output('7')?.provenance.kind).toBe('workflow-snapshot');
  expect(f.runtime.outputs.output('999')).toBeNull();
  f.runtime.dispose();
});

test('same-workflow reload invalidates outstanding jobs and rejects their late media', async () => {
  const f = fixture();
  f.runtime.install();
  f.bridge.install();
  await f.api.queuePrompt(0, await f.host.graphToPrompt());
  f.api.emit('executed', {
    prompt_id: 'job-1',
    node: '7',
    output: { images: [{ filename: 'old.png' }] },
  });
  await f.host.loadGraphData({}, true, true, f.a);
  f.api.emit('executed', {
    prompt_id: 'job-1',
    node: '7',
    output: { images: [{ filename: 'late.png' }] },
  });
  expect(f.runtime.outputs.output('7')).toBeNull();
  f.bridge.dispose();
  f.runtime.dispose();
});

test('inactive preview consumers receive no bytes and preview projections never use another tab global map', async () => {
  const f = fixture();
  f.runtime.install();
  f.bridge.install();
  const a = f.runtime.sessions.read().active;
  if (!a) throw new Error('Missing workflow A');
  await f.api.queuePrompt(0, await f.host.graphToPrompt());
  const frames: Blob[] = [];
  const unsubscribe = f.runtime.previews.subscribe({
    wants: (source) => f.runtime.sessions.read().active?.sessionId === source.run.session.sessionId,
    receive: (_preview, blob) => {
      frames.push(blob);
    },
  });
  await f.host.loadGraphData({}, true, true, f.b);
  f.host.nodePreviewImages = { '42:23': ['blob:foreign'] };
  f.api.emit('b_preview_with_metadata', {
    jobId: 'job-1',
    nodeId: '42:23',
    blob: new Blob(['background']),
  });
  expect(frames).toEqual([]);
  expect(f.runtime.outputs.livePreview('42:23')).toEqual([]);
  await f.host.loadGraphData({}, true, true, f.a);
  const stopProjection = f.runtime.outputs.subscribe(() => undefined);
  expect(f.runtime.outputs.livePreview('42:23')).toEqual([]);
  expect(f.created).toEqual([]);
  const current = new Blob(['current']);
  f.api.emit('b_preview_with_metadata', { jobId: 'job-1', nodeId: '42:23', blob: current });
  expect(frames).toEqual([current]);
  expect(f.runtime.outputs.livePreview('42:23')).toEqual(['blob:owned-1']);
  stopProjection();
  unsubscribe();
  f.bridge.dispose();
  f.runtime.dispose();
  expect(f.revoked).toEqual(['blob:owned-1']);
});

test('refuses observed construction from another active workflow before live-graph preparation', async () => {
  const f = fixture();
  f.runtime.install();
  f.bridge.install();
  const a = f.runtime.sessions.read().active;
  if (!a) throw new Error('Missing workflow A');
  const completion: { release: (() => void) | null } = { release: null };
  f.host.pauseConstruction = () =>
    new Promise<void>((resolve) => {
      completion.release = resolve;
    });
  const payload = f.host.graphToPrompt();
  await f.host.loadGraphData({}, true, true, f.b);
  const release = completion.release;
  if (!release) throw new Error('Missing construction pause');
  release();
  await expect(f.api.queuePrompt(0, await payload)).rejects.toThrow('workflow changed');
  f.api.emit('executed', {
    prompt_id: 'job-1',
    node: '7',
    output: { images: [{ filename: 'entry.png' }] },
  });
  expect(f.runtime.results.read(a.sessionId)?.outputs).toEqual([]);
  expect(f.runtime.outputs.output('7')).toBeNull();
  f.bridge.dispose();
  f.runtime.dispose();
});

test('preserves a submitted job owner when its transport settles after a tab switch', async () => {
  const f = fixture();
  f.runtime.install();
  f.bridge.install();
  const a = f.runtime.sessions.read().active;
  if (!a) throw new Error('Missing workflow');
  const completion: { release: (() => void) | null } = { release: null };
  f.api.pauseTransport = () =>
    new Promise<void>((resolve) => {
      completion.release = resolve;
    });
  const queued = f.api.queuePrompt(0, await f.host.graphToPrompt());
  await settleWorkflowHistory();
  await f.host.loadGraphData({}, true, true, f.b);
  const release = completion.release;
  if (!release) throw new Error('Missing transport pause');
  release();
  await queued;
  f.api.emit('executed', {
    prompt_id: 'job-1',
    node: '7',
    output: { images: [{ filename: 'submitted-a.png' }] },
  });
  expect(f.runtime.results.read(a.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'submitted-a.png',
  );
  expect(f.runtime.outputs.output('7')).toBeNull();
  f.bridge.dispose();
  f.runtime.dispose();
});

test('preserves unobserved public queue callers without attributing their output to an active tab', async () => {
  const f = fixture();
  f.runtime.install();
  f.bridge.install();
  await expect(
    f.api.queuePrompt(0, { output: { '7': { class_type: 'PreviewImage' } } }),
  ).resolves.toEqual({ prompt_id: 'job-1' });
  f.api.emit('executed', {
    prompt_id: 'job-1',
    node: '7',
    output: { images: [{ filename: 'external.png' }] },
  });
  expect(f.runtime.outputs.output('7')).toBeNull();
  f.bridge.dispose();
  f.runtime.dispose();
});

test.each(['node', 'display_node'])(
  'captures prompt-only cube marker media reported through %s and exact job history',
  async (field) => {
    const f = fixture();
    f.runtime.install();
    f.bridge.install();
    const marker = buildCubeOutputExecutionId('42', 0);
    const payload = await f.host.graphToPrompt();
    if (!('output' in payload) || typeof payload.output !== 'object' || payload.output === null)
      throw new Error('Missing prompt');
    Object.assign(payload.output, { [marker]: { class_type: 'SugarCubes.CubeOutput' } });
    await f.api.queuePrompt(0, payload);
    f.api.emit('executed', {
      prompt_id: 'job-1',
      node: '42:23',
      [field]: marker,
      output: { images: [{ filename: 'marker.png', type: 'temp' }], text: ['metadata'] },
    });
    expect(f.runtime.outputs.output(marker)?.items[0]?.file.filename).toBe('marker.png');
    expect(f.runtime.outputs.output(marker)?.text).toEqual(['metadata']);
    f.api.emit('execution_success', { prompt_id: 'job-1' });
    expect(f.api.history.request(0).path).toBe('/history/job-1');
    f.api.history.request(0).respond({
      'job-1': {
        outputs: {
          [marker]: { images: [{ filename: 'marker.png', type: 'temp' }], text: ['metadata'] },
        },
      },
    });
    await settleWorkflowHistory();
    expect(f.runtime.outputs.output(marker)?.items).toHaveLength(1);
    f.bridge.dispose();
    f.runtime.dispose();
    expect([...f.api.listeners.values()].every((listeners) => listeners.size === 0)).toBe(true);
  },
);

test('admits one requested cube preview at fifteen frames per second and releases it on tab suspension', async () => {
  const f = fixture();
  f.runtime.install();
  f.bridge.install();
  await f.api.queuePrompt(0, await f.host.graphToPrompt());
  f.api.emit('b_preview_with_metadata', {
    jobId: 'job-1',
    nodeId: '42:23',
    blob: new Blob(['unrequested']),
  });
  expect(f.created).toHaveLength(0);
  const stop = f.runtime.outputs.subscribe(() => undefined);
  f.runtime.outputs.livePreview('42:23');
  for (let i = 0; i < 50; i++)
    f.api.emit('b_preview_with_metadata', {
      jobId: 'job-1',
      nodeId: '42:23',
      blob: new Blob([String(i)]),
    });
  expect(f.created).toHaveLength(1);
  expect(f.revoked).toEqual([]);
  f.clock.advance(67);
  f.api.emit('b_preview_with_metadata', {
    jobId: 'job-1',
    nodeId: '42:23',
    blob: new Blob(['next']),
  });
  expect(f.created).toHaveLength(2);
  expect(f.revoked).toEqual(['blob:owned-1']);
  await f.host.loadGraphData({}, true, true, f.b);
  expect(f.revoked).toEqual(['blob:owned-1', 'blob:owned-2']);
  f.clock.advance(67);
  f.api.emit('b_preview_with_metadata', {
    jobId: 'job-1',
    nodeId: '42:23',
    blob: new Blob(['background']),
  });
  expect(f.created).toHaveLength(2);
  stop();
  f.bridge.dispose();
  f.runtime.dispose();
});

test('closing a workflow cancels its recovery and rejects late events before a fresh reopen', async () => {
  const f = fixture();
  f.runtime.install();
  f.bridge.install();
  const old = f.runtime.sessions.read().active;
  if (!old) throw new Error('Missing workflow');
  await f.api.queuePrompt(0, await f.host.graphToPrompt());
  f.api.emit('execution_success', { prompt_id: 'job-1' });
  const request = f.api.history.request(0);
  f.host.close(f.a);
  expect(request.signal.aborted).toBe(true);
  expect(f.runtime.results.read(old.sessionId)).toBeNull();
  await f.host.loadGraphData({}, true, true, f.a);
  expect(f.runtime.sessions.read().active?.sessionId).not.toBe(old.sessionId);
  request.respond({
    'job-1': { outputs: { '7': { images: [{ filename: 'late-history.png' }] } } },
  });
  f.api.emit('executed', {
    prompt_id: 'job-1',
    node: '7',
    output: { images: [{ filename: 'late-event.png' }] },
  });
  await settleWorkflowHistory();
  expect(f.runtime.outputs.output('7')).toBeNull();
  f.bridge.dispose();
  f.runtime.dispose();
});

test('releases cube preview bytes when the browser document is hidden and resumes only with a new frame', async () => {
  const f = fixture();
  f.runtime.install();
  f.bridge.install();
  await f.api.queuePrompt(0, await f.host.graphToPrompt());
  const stop = f.runtime.outputs.subscribe(() => undefined);
  f.runtime.outputs.livePreview('42:23');
  const emit = () =>
    f.api.emit('b_preview_with_metadata', {
      jobId: 'job-1',
      nodeId: '42:23',
      blob: new Blob(['frame']),
    });
  emit();
  expect(f.created).toHaveLength(1);
  f.visibility.visible = false;
  for (const listener of f.visibility.listeners) listener();
  expect(f.revoked).toEqual(['blob:owned-1']);
  emit();
  expect(f.created).toHaveLength(1);
  f.visibility.visible = true;
  for (const listener of f.visibility.listeners) listener();
  expect(f.runtime.outputs.livePreview('42:23')).toEqual([]);
  emit();
  expect(f.created).toHaveLength(2);
  stop();
  f.bridge.dispose();
  f.runtime.dispose();
  expect(f.visibility.listeners.size).toBe(0);
});
