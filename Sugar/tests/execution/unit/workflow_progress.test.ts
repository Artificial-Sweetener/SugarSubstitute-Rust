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
/** Prove job-qualified progress counts submitted nodes and preserves workflow ownership. */
import { WorkflowProgressStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowProgressStore.js';
import {
  promptId,
  outputSourceId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import { workflowResultsFixture } from '../support/workflow_results.js';
import {
  applyComfySamplerProgress,
  applyComfyProgressState,
} from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowProgressDecoder.js';

/** Retain two actual submitted paths independently of the server's non-pending subset. */
function fixture() {
  const f = workflowResultsFixture();
  const source = f.a.sources[0];
  if (!source) throw new Error('Missing submitted source');
  const context = {
    ...f.a,
    sources: [source, { ...source, id: outputSourceId('second'), executionPath: 'second' }],
  };
  const progress = new WorkflowProgressStore((id) => f.runs.read(id));
  return {
    ...f,
    context,
    progress,
    accept: () => f.ingestor.accepted(f.ingestor.submitting(context), f.id),
  };
}

test('early progress becomes visible only after its exact run is accepted', () => {
  const f = fixture();
  f.progress.started(f.id);
  f.progress.sampler(f.id, '42:23', 4, 10);
  expect(f.progress.read(f.a.session).visible).toBe(false);
  f.accept();
  expect(f.progress.read(f.a.session)).toEqual({ visible: true, workflow: 0, sampler: 40 });
  expect(f.progress.read(f.b.session).visible).toBe(false);
  expect(f.progress.read({ ...f.a.session, generation: 1 }).visible).toBe(false);
});

test('workflow total comes from submitted paths and completed paths count once', () => {
  const f = fixture();
  f.accept();
  f.progress.started(f.id);
  applyComfyProgressState(
    { detail: { prompt_id: 'one', nodes: { '42:23': { state: 'finished', value: 10, max: 10 } } } },
    f.progress,
  );
  f.progress.completed(f.id, ['42:23', 'not-submitted', '42:23']);
  expect(f.progress.read(f.a.session).workflow).toBe(50);
  f.progress.completed(f.id, ['second']);
  expect(f.progress.read(f.a.session).workflow).toBe(100);
});

test('sampler resets between nodes and refuses malformed or stale events', () => {
  const f = fixture();
  f.accept();
  f.progress.started(f.id);
  applyComfySamplerProgress(
    { detail: { prompt_id: 'one', node: '42:23', value: 8, max: 10 } },
    f.progress,
  );
  expect(f.progress.read(f.a.session).sampler).toBe(80);
  for (const detail of [
    { prompt_id: 'foreign', node: '42:23', value: 10, max: 10 },
    { prompt_id: 'one', node: '42:23', value: Number.NaN, max: 10 },
    { prompt_id: 'one', node: '42:23', value: 4, max: 0 },
    { node: '42:23', value: 9, max: 10 },
  ])
    applyComfySamplerProgress({ detail }, f.progress);
  expect(f.progress.read(f.a.session).sampler).toBe(80);
  f.progress.nodeFinished(f.id, ['42:23']);
  expect(f.progress.read(f.a.session).sampler).toBe(0);
  f.progress.sampler(f.id, 'second', 1, 5);
  f.progress.terminal(promptId('foreign'));
  expect(f.progress.read(f.a.session).sampler).toBe(20);
  f.progress.terminal(f.id);
  applyComfySamplerProgress(
    { detail: { prompt_id: 'one', node: 'second', value: 5, max: 5 } },
    f.progress,
  );
  expect(f.progress.read(f.a.session).visible).toBe(false);
});

test('retirement releases progress observers and retained execution state', () => {
  const f = fixture();
  f.accept();
  let changes = 0;
  const release = f.progress.subscribe(() => changes++);
  f.progress.started(f.id);
  expect(changes).toBe(1);
  release();
  f.progress.reset();
  expect(changes).toBe(1);
  f.progress.dispose();
  expect(f.progress.read(f.a.session).visible).toBe(false);
});
