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
/** Qualify durable identities against reused nodes, changed labels and graph revisions. */
import { WorkflowOutputIdentities } from '../../../frontend/comfyui/ui/execution/application/WorkflowOutputIdentities.js';
import { outputSourceId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { SessionBinding } from '../../../integrations/contracts/SessionProtocol.generated.js';

test('submitted source snapshot is immutable and a later graph revision cannot redefine its identity', () => {
  let sequence = 10;
  const identities = new WorkflowOutputIdentities(
    () => `00000000-0000-4000-8000-${(++sequence).toString(16).padStart(12, '0')}`,
  );
  const binding: SessionBinding = {
    entryId: '00000000-0000-4000-8000-000000000001',
    runtimeSession: 'fresh',
    generation: 0,
    graphRevision: 3,
    sourceRevision: 1,
    navigationRevision: 1,
  };
  const source = {
    id: outputSourceId('42:23'),
    executionPath: '42:23:5',
    label: 'Original',
    breadcrumb: [],
    order: 0,
  };
  const first = identities.sources(binding, [source])[0];
  if (!first) throw new Error('Submitted source fixture was not captured.');
  expect(first.nodeId).toBe('42:23');
  expect(identities.sources(binding, [{ ...source, label: 'Changed' }])[0]).toBe(first);
  const replacement = identities.sources({ ...binding, graphRevision: 4 }, [
    { ...source, label: 'Changed' },
  ])[0];
  if (!replacement) throw new Error('Replacement source fixture was not captured.');
  expect(replacement.id).not.toBe(first.id);
  expect(first.label).toBe('Original');
  const restored = new WorkflowOutputIdentities(() => '00000000-0000-4000-8000-000000000099');
  restored.restoreSources(binding.entryId, [first]);
  expect(restored.sources(binding, [source])[0]?.id).toBe(first.id);
  expect(identities.run(binding.entryId, 'prompt')).toBe(identities.run(binding.entryId, 'prompt'));
  expect(identities.output(binding.entryId, 'runtime-output')).toBe(
    identities.output(binding.entryId, 'runtime-output'),
  );
});
