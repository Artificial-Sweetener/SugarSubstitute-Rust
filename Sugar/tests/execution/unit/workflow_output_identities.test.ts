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
import { decodeComfyPromptSources } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptSourceDecoder.js';
import { buildCubeOutputExecutionId } from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';
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

test('a Cube output attributed from every node inside the Cube is one durable source, at the path that delivers its finals', () => {
  let sequence = 20;
  const identities = new WorkflowOutputIdentities(
    () => `00000000-0000-4000-8000-${(++sequence).toString(16).padStart(12, '0')}`,
  );
  const binding: SessionBinding = {
    entryId: '00000000-0000-4000-8000-000000000002',
    runtimeSession: 'fresh',
    generation: 0,
    graphRevision: 5,
    sourceRevision: 1,
    navigationRevision: 1,
  };
  const marker = buildCubeOutputExecutionId(23, 0);
  const attributed = decodeComfyPromptSources({
    output: {
      'instance:sampler': { class_type: 'KSampler' },
      'instance:decode': { class_type: 'VAEDecode' },
      [marker]: {
        class_type: 'SugarCubes.CubeOutput',
        inputs: { instance_id: 'instance', value: ['instance:decode', 0] },
      },
      '9': { class_type: 'SaveImage' },
    },
  });
  expect(attributed.map((source) => source.id)).toEqual([
    outputSourceId('9'),
    outputSourceId(marker),
    outputSourceId(marker),
    outputSourceId(marker),
  ]);
  const captured = identities.sources(binding, attributed);
  expect(captured.map((source) => [source.nodeId, source.executionPath])).toEqual([
    ['9', '9'],
    [marker, marker],
  ]);
  expect(new Set(captured.map((source) => source.id)).size).toBe(2);
  expect(identities.inventory(binding.entryId)).toEqual(captured);
  const stage = attributed.find((source) => source.executionPath === 'instance:sampler');
  if (!stage) throw new Error('Cube stage attribution fixture is missing.');
  expect(identities.sources(binding, [stage])).toEqual([captured[1]]);
});

test('a source known only through routed paths is captured at the path whose finals it accepts', () => {
  const identities = new WorkflowOutputIdentities(() => '00000000-0000-4000-8000-000000000031');
  const binding: SessionBinding = {
    entryId: '00000000-0000-4000-8000-000000000003',
    runtimeSession: 'fresh',
    generation: 0,
    graphRevision: 1,
    sourceRevision: 1,
    navigationRevision: 1,
  };
  const id = outputSourceId(buildCubeOutputExecutionId(4, 0));
  const routed = { id, label: 'image', breadcrumb: [], order: 0 };
  expect(
    identities.sources(binding, [
      { ...routed, executionPath: 'instance:sampler', transientOnly: true },
      { ...routed, executionPath: 'instance:decode' },
    ]),
  ).toMatchObject([{ executionPath: 'instance:decode' }]);
});
