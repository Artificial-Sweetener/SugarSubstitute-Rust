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
/** Prove workflow-owned source metadata follows complete instance paths and authored labels. */

import { decodeComfyWorkflowSources } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowSourceDecoder.js';
import { decodeComfyPromptSources } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyPromptSourceDecoder.js';
import { buildCubeOutputExecutionId } from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';
import { outputSourceId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';

test('gives repeated definition instances distinct full paths and literal named breadcrumbs', () => {
  const workflow = {
    nodes: [
      { id: 42, type: 'definition', title: 'First <b>' },
      { id: 43, type: 'definition', title: 'Second' },
    ],
    definitions: {
      subgraphs: [
        {
          id: 'definition',
          name: 'Detail',
          nodes: [{ id: 23, type: 'PreviewImage', title: 'Final' }],
        },
      ],
    },
  };
  const sources = decodeComfyWorkflowSources(workflow);
  expect(sources.map((source) => [source.executionPath, source.breadcrumb])).toEqual([
    ['42', []],
    ['42:23', ['First <b>']],
    ['43', []],
    ['43:23', ['Second']],
  ]);
  const prompt = decodeComfyPromptSources({
    workflow,
    output: { '43:23': { class_type: 'PreviewImage' }, '42:23': { class_type: 'PreviewImage' } },
  });
  expect(prompt.find((source) => source.executionPath === '42:23')).toMatchObject({
    label: 'Final',
    breadcrumb: ['First <b>'],
    order: 1,
  });
  expect(prompt.find((source) => source.executionPath === '43:23')?.order).toBe(3);
});

test('projects cube sink identities with the existing identity owner and authored output names', () => {
  const workflow = {
    nodes: [
      {
        id: 5,
        title: 'Text to image',
        type: 'cube-definition',
        properties: { sugarcubes_kind: 'cube' },
      },
    ],
    definitions: {
      subgraphs: [
        { id: 'cube-definition', outputs: [{ name: 'Image' }, { name: 'Mask' }], nodes: [] },
      ],
    },
  };
  const marker = buildCubeOutputExecutionId(5, 1);
  const metadata = decodeComfyWorkflowSources(workflow);
  expect(metadata.find((source) => source.executionPath === marker)).toMatchObject({
    label: 'Mask',
    breadcrumb: ['Text to image'],
  });
  expect(
    decodeComfyPromptSources(
      { output: { [marker]: { class_type: 'SugarCubes.CubeOutput' } } },
      metadata,
    )[0]?.label,
  ).toBe('Mask');
});

test('rejects malformed collection values and terminates cyclic definition references', () => {
  expect(decodeComfyWorkflowSources([])).toEqual([]);
  expect(decodeComfyPromptSources({ output: [{ class_type: 'PreviewImage' }] })).toEqual([]);
  const graph = { id: 'cycle', nodes: [{ id: 1, type: 'cycle' }] };
  const workflow = { nodes: [{ id: 1, type: 'cycle' }], definitions: { subgraphs: [graph] } };
  expect(decodeComfyWorkflowSources(workflow).map((source) => source.executionPath)).toEqual([
    '1',
    '1:1',
  ]);
});

test('routes compiled Cube internals to the exposed stage while preserving authored source order', () => {
  const marker = buildCubeOutputExecutionId(23, 0);
  const sources = decodeComfyPromptSources({
    output: {
      'instance:sampler': { class_type: 'KSampler' },
      'instance:decode': { class_type: 'VAEDecode' },
      [marker]: {
        class_type: 'SugarCubes.CubeOutput',
        inputs: { instance_id: 'instance', value: ['instance:decode', 0] },
      },
      'other:decode': { class_type: 'PreviewImage' },
    },
  });
  expect(sources.find((source) => source.executionPath === 'instance:sampler')).toMatchObject({
    id: outputSourceId(marker),
    transientOnly: true,
  });
  expect(sources.find((source) => source.executionPath === 'instance:decode')).toMatchObject({
    id: outputSourceId(marker),
  });
  expect(sources.find((source) => source.executionPath === 'instance:decode')).not.toHaveProperty(
    'transientOnly',
  );
  expect(sources.find((source) => source.executionPath === 'other:decode')?.id).toBe(
    outputSourceId('other:decode'),
  );
  expect(sources.find((source) => source.executionPath === marker)?.order).toBe(
    sources.find((source) => source.executionPath === 'instance:sampler')?.order,
  );
});

test('does not guess between two Cube outputs or accept foreign bound producer identities', () => {
  const first = buildCubeOutputExecutionId(23, 0);
  const second = buildCubeOutputExecutionId(23, 1);
  const sources = decodeComfyPromptSources({
    output: {
      'instance:sampler': { class_type: 'KSampler' },
      'instance:a': { class_type: 'VAEDecode' },
      'instance:b': { class_type: 'VAEDecode' },
      [first]: {
        class_type: 'SugarCubes.CubeOutput',
        inputs: { instance_id: 'instance', value: ['instance:a', 0] },
      },
      [second]: {
        class_type: 'SugarCubes.CubeOutput',
        inputs: { instance_id: 'instance', value: ['instance:b', 0] },
      },
    },
  });
  expect(sources.find((source) => source.executionPath === 'instance:sampler')?.id).toBe(
    outputSourceId('instance:sampler'),
  );
  expect(sources.find((source) => source.executionPath === 'instance:a')?.id).toBe(
    outputSourceId(first),
  );
  expect(sources.find((source) => source.executionPath === 'instance:b')?.id).toBe(
    outputSourceId(second),
  );
  const foreign = decodeComfyPromptSources({
    output: {
      'instance:a': { class_type: 'VAEDecode' },
      [first]: {
        class_type: 'SugarCubes.CubeOutput',
        inputs: { instance_id: 'instance', value: ['foreign:a', 0] },
      },
    },
  });
  expect(foreign.find((source) => source.executionPath === 'instance:a')?.id).toBe(
    outputSourceId('instance:a'),
  );
});

test('generates distinct opaque session identities without the secure-context UUID method', async () => {
  const { workflowSessionKey } = await import(
    '../../../frontend/comfyui/ui/execution/infrastructure/WorkflowSessionKey.js'
  );
  const keys = Array.from({ length: 100 }, () => workflowSessionKey());
  expect(keys.every((key) => /^[0-9a-f]{32}$/.test(key))).toBe(true);
  expect(new Set(keys).size).toBe(100);
});
