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

/** Bind the real event adapter to observable preview and diagnostic boundaries. */
function fixture() {
  const f = workflowResultsFixture();
  const api = new WorkflowEventApi();
  const previews: Blob[] = [];
  const errors: unknown[] = [];
  const connections: boolean[] = [];
  let reconnects = 0;
  const adapter = new ComfyWorkflowEventAdapter({
    api,
    ingestor: f.ingestor,
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
  return { ...f, api, adapter, previews, errors, connections, reconnects: () => reconnects };
}

test('delivers enhanced preview bytes once and refuses to guess from an unassigned enhanced delivery', () => {
  const f = fixture();
  f.adapter.install();
  f.api.emit('execution_start', { prompt_id: 'one' });
  f.api.emit('executing', '42:23');
  const enhanced = new Blob(['image']);
  f.api.emit('b_preview_with_metadata', { jobId: 'one', nodeId: '42:23', blob: enhanced });
  f.api.emit('b_preview', enhanced);
  expect(f.previews).toEqual([enhanced]);
  const unassigned = new Blob(['foreign']);
  f.api.emit('b_preview_with_metadata', { jobId: 'foreign', nodeId: '42:23', blob: unassigned });
  f.api.emit('b_preview', unassigned);
  expect(f.previews).toEqual([enhanced]);
  const legacy = new Blob(['legacy']);
  f.api.emit('b_preview', legacy);
  expect(f.previews).toEqual([enhanced]);
  f.api.emit('execution_success', { prompt_id: 'one' });
  f.api.emit('b_preview_with_metadata', { jobId: 'one', nodeId: '42:23', blob: new Blob() });
  expect(f.previews).toHaveLength(1);
  f.adapter.dispose();
});

test('never assigns delayed unqualified bytes to a newer executing generation', () => {
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
  expect(f.previews).toEqual([]);
  f.api.emit('b_preview_with_metadata', { jobId: 'one', nodeId: '42:23', blob: old });
  f.api.emit('b_preview', new Blob(['previous-generation']));
  expect(f.previews).toEqual([]);
  const current = new Blob(['current-generation']);
  f.api.emit('b_preview_with_metadata', { jobId: 'two', nodeId: '42:23', blob: current });
  expect(f.previews).toEqual([current]);
  f.adapter.dispose();
});

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

test('registers once, preserves unrelated listeners, and stops deliveries after disposal and reinstall', () => {
  const f = fixture();
  let unrelated = 0;
  f.api.addEventListener('reconnected', () => {
    unrelated++;
  });
  f.adapter.install();
  f.adapter.install();
  f.api.emit('disconnected', null);
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
});

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
