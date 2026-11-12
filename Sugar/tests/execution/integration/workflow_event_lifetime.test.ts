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
/** Verify public execution event adaptation and exact subscription teardown. */

import { ComfyWorkflowEventAdapter } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowEventAdapter.js';
import { WorkflowEventApi, workflowResultsFixture } from '../support/workflow_results.js';
import { promptId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { WorkflowProgressStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowProgressStore.js';

/** Bind the real event adapter to observable preview and diagnostic boundaries. */
function fixture() {
  const f = workflowResultsFixture();
  const api = new WorkflowEventApi();
  const previews: Blob[] = [];
  const errors: unknown[] = [];
  const connections: boolean[] = [];
  let reconnects = 0;
  const progress = new WorkflowProgressStore((id) => f.runs.read(id));
  const adapter = new ComfyWorkflowEventAdapter({
    api,
    ingestor: f.ingestor,
    progress,
    logger: {
      error: (...args: unknown[]) => {
        errors.push(args);
      },
    },
    preview: (_source, blob) => {
      previews.push(blob);
    },
    connection: (connected) => {
      connections.push(connected);
    },
    reconnect: () => {
      reconnects++;
    },
  });
  f.ingestor.accepted(f.ingestor.submitting(f.a), f.id);
  return {
    ...f,
    api,
    adapter,
    progress,
    previews,
    errors,
    connections,
    reconnects: () => reconnects,
  };
}

test('public execution events keep workflow and sampler progress separate and retire terminal jobs', () => {
  const f = fixture();
  f.adapter.install();
  f.api.emit('execution_start', { prompt_id: 'one' });
  f.api.emit('progress', { prompt_id: 'one', node: '42:23', value: 4, max: 10 });
  expect(f.progress.read(f.a.session)).toEqual({ visible: true, workflow: 0, sampler: 40 });
  f.api.emit('executed', { prompt_id: 'one', node: '42:23' });
  expect(f.progress.read(f.a.session)).toEqual({ visible: true, workflow: 100, sampler: 0 });
  f.api.emit('execution_success', { prompt_id: 'one' });
  expect(f.progress.read(f.a.session).visible).toBe(false);
  f.adapter.dispose();
});

test.each(['promptId', 'jobId'] as const)(
  'delivers %s preview bytes once and refuses anonymous or terminal deliveries',
  (field) => {
    const f = fixture();
    f.adapter.install();
    f.api.emit('execution_start', { prompt_id: 'one' });
    f.api.emit('executing', '42:23');
    const enhanced = new Blob(['image']);
    f.api.emit('b_preview_with_metadata', { [field]: 'one', nodeId: '42:23', blob: enhanced });
    f.api.emit('b_preview', enhanced);
    expect(f.previews).toHaveLength(1);
    expect(f.previews[0]).toBe(enhanced);
    const unassigned = new Blob(['foreign']);
    f.api.emit('b_preview_with_metadata', {
      [field]: 'foreign',
      nodeId: '42:23',
      blob: unassigned,
    });
    f.api.emit('b_preview', unassigned);
    expect(f.previews).toHaveLength(1);
    expect(f.previews[0]).toBe(enhanced);
    const legacy = new Blob(['legacy']);
    f.api.emit('b_preview', legacy);
    expect(f.previews).toHaveLength(1);
    expect(f.previews[0]).toBe(enhanced);
    f.api.emit('execution_success', { prompt_id: 'one' });
    f.api.emit('b_preview_with_metadata', { [field]: 'one', nodeId: '42:23', blob: new Blob() });
    expect(f.previews).toHaveLength(1);
    f.adapter.dispose();
  },
);

test.each([
  { name: 'undefined', value: undefined },
  { name: 'null', value: null },
  { name: 'number', value: 1 },
  { name: 'object', value: {} },
  { name: 'array', value: [] },
  { name: 'empty', value: '' },
  { name: 'whitespace', value: ' ' },
  { name: 'overlong', value: 'x'.repeat(4097) },
  { name: 'foreign', value: 'foreign' },
])('refuses $name metadata without falling back to the other accepted host field', ({ value }) => {
  const f = fixture();
  f.adapter.install();
  f.api.emit('execution_start', { prompt_id: 'one' });
  f.api.emit('b_preview_with_metadata', {
    promptId: value,
    jobId: 'one',
    nodeId: '42:23',
    blob: new Blob(['unqualified']),
  });
  f.api.emit('b_preview_with_metadata', {
    promptId: 'one',
    jobId: value,
    nodeId: '42:23',
    blob: new Blob(['unqualified-job']),
  });
  for (const field of ['promptId', 'jobId'])
    f.api.emit('b_preview_with_metadata', {
      [field]: value,
      nodeId: '42:23',
      blob: new Blob(['unqualified-only']),
    });
  expect(f.previews).toHaveLength(0);
  expect(f.errors).toEqual([]);
  f.adapter.dispose();
});

test('accepts matching host metadata identities and rejects missing or conflicting identities', () => {
  const f = fixture();
  f.adapter.install();
  f.api.emit('execution_start', { prompt_id: 'one' });
  f.api.emit('b_preview_with_metadata', {
    nodeId: '42:23',
    blob: new Blob(['anonymous']),
  });
  expect(f.previews).toHaveLength(0);
  f.api.emit('b_preview_with_metadata', {
    promptId: 'one',
    jobId: 'foreign',
    nodeId: '42:23',
    blob: new Blob(['conflicting']),
  });
  expect(f.previews).toHaveLength(0);
  const actual = new Blob(['matching-prompt']);
  f.api.emit('b_preview_with_metadata', {
    promptId: 'one',
    jobId: 'one',
    nodeId: '42:23',
    blob: actual,
  });
  expect(f.previews).toHaveLength(1);
  expect(f.previews[0]).toBe(actual);
  expect(f.errors).toEqual([]);
  f.adapter.dispose();
});

test.each(['promptId', 'jobId'] as const)(
  'never assigns delayed %s bytes to a newer executing generation',
  (field) => {
    const f = fixture();
    f.adapter.install();
    f.api.emit('execution_start', { prompt_id: 'one' });
    f.api.emit('executing', '42:23');
    const old = new Blob(['previous-generation']);
    f.api.emit('execution_interrupted', { prompt_id: 'one' });
    f.ingestor.accepted(f.ingestor.submitting(f.a), promptId('two'));
    f.api.emit('execution_start', { prompt_id: 'two' });
    f.api.emit('executing', '42:23');
    f.api.emit('b_preview', old);
    expect(f.previews).toHaveLength(0);
    f.api.emit('b_preview_with_metadata', { [field]: 'one', nodeId: '42:23', blob: old });
    f.api.emit('b_preview', new Blob(['previous-generation']));
    expect(f.previews).toHaveLength(0);
    const current = new Blob(['current-generation']);
    f.api.emit('b_preview_with_metadata', { [field]: 'two', nodeId: '42:23', blob: current });
    expect(f.previews).toHaveLength(1);
    expect(f.previews[0]).toBe(current);
    f.adapter.dispose();
  },
);

test('adapts ordinary and nested finals while rejecting invalid payloads and unqualified node paths', () => {
  const f = fixture();
  f.adapter.install();
  f.api.emit('executed', {
    prompt_id: 'one',
    node: '42:23',
    output: { images: [{ filename: 'nested.png' }] },
  });
  f.api.emit('executed', {
    prompt_id: 'one',
    node: '23',
    output: { images: [{ filename: 'wrong.png' }] },
  });
  f.api.emit('executed', { prompt_id: 'one', node: '42:23', output: [], merge: 'true' });
  expect(f.results.read(f.a.session.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe(
    'nested.png',
  );
  f.adapter.dispose();
});

test.each(['disconnected', 'reconnecting'])(
  'tracks host connection loss and preserves exact listener lifetime: %s',
  (connectionLost) => {
    const f = fixture();
    let unrelated = 0;
    f.api.addEventListener('reconnected', () => {
      unrelated++;
    });
    f.adapter.install();
    f.adapter.install();
    f.api.emit(connectionLost, null);
    f.api.emit('reconnected', null);
    expect(f.connections).toEqual([false, true]);
    expect(f.reconnects()).toBe(1);
    f.adapter.dispose();
    f.api.emit('reconnected', null);
    expect(f.reconnects()).toBe(1);
    expect(unrelated).toBe(2);
    f.adapter.install();
    f.api.emit('reconnected', null);
    expect(f.reconnects()).toBe(2);
    f.adapter.dispose();
    expect(
      [...f.api.listeners.values()].reduce((count, listeners) => count + listeners.size, 0),
    ).toBe(1);
  },
);

test('rolls back registration that throws after the host retained a listener', () => {
  const f = fixture();
  f.api.failAdd = 'progress';
  expect(() => f.adapter.install()).toThrow('Host registration failed');
  expect([...f.api.listeners.values()].every((listeners) => listeners.size === 0)).toBe(true);
  f.api.failAdd = null;
  f.adapter.install();
  f.adapter.dispose();
});

test('continues teardown after a host removal failure and disables any retained callback', () => {
  const f = fixture();
  f.adapter.install();
  f.api.failRemove = 'execution_start';
  f.adapter.dispose();
  expect(f.errors).toHaveLength(1);
  expect(f.api.listeners.get('reconnected')?.size).toBe(0);
  f.api.emit('execution_start', { prompt_id: 'one' });
  expect(f.runs.read(f.id)?.status).toBe('queued');
});
