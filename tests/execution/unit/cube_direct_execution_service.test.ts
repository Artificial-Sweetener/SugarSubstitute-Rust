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
//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
/** Verify browser request projection for authoritative Cube execution. */

import { jest } from '@jest/globals';
import { CubeDirectExecutionService } from '../../../frontend/comfyui/ui/cube/execution/CubeDirectExecutionService.js';
import type { CubeExecutionApiClient } from '../../../frontend/comfyui/ui/cube/execution/CubeExecutionApiClient.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';
import type { ProximityMatch } from '../../../frontend/comfyui/ui/overlays/proximity/ProximityModel.js';

test('serializes embedded workflow state and passes only Cube-to-Cube proximity', async () => {
  const queue = jest.fn(async () => ({ accepted: true }));
  const acceptedMatches: ProximityMatch[] = [];
  const source = cube('cube-a', ['output.image'], []);
  const target = cube('cube-b', [], ['input.image']);
  const graph = {
    _nodes: [source, target],
    _subgraphs: new Map(),
    serialize: () => {
      acceptedMatches.length = 0;
      return { nodes: [], definitions: { subgraphs: [] } };
    },
  } as unknown as ComfyGraph;
  const match = {
    outputCube: 'source-definition-id',
    inputCube: 'target-definition-id',
    outputInstanceId: 'cube-a',
    outputBinding: 'output.image',
    inputInstanceId: 'cube-b',
    inputBinding: 'input.image',
    outputSlot: 0,
    inputSlot: 0,
  } as unknown as ProximityMatch;
  acceptedMatches.push(match);
  const service = new CubeDirectExecutionService({
    client: { queue } as unknown as CubeExecutionApiClient,
    hostApi: {
      clientId: 'host-client',
      authToken: 'secret-token',
      apiKey: 'secret-key',
    },
    getGraph: () => graph,
    getProximityMatches: () => acceptedMatches,
  });

  await service.queue(
    7,
    {
      extra_data: { caller: 'test' },
    },
    {
      partialExecutionTargets: ['cube-b'],
      previewMethod: 'latent2rgb',
    },
  );

  expect(queue).toHaveBeenCalledWith(
    expect.objectContaining({
      schema_version: 1,
      proximity_connections: [
        {
          source_instance_id: 'cube-a',
          source_binding: 'output.image',
          target_instance_id: 'cube-b',
          target_binding: 'input.image',
        },
      ],
      queue: expect.objectContaining({
        client_id: 'host-client',
        front: false,
        number: 7,
        partial_execution_targets: ['cube-b'],
      }),
      extra_data: expect.objectContaining({
        caller: 'test',
        comfy_usage_source: 'comfyui-frontend',
        extra_pnginfo: { workflow: expect.any(Object) },
        auth_token_comfy_org: 'secret-token',
        api_key_comfy_org: 'secret-key',
        preview_method: 'latent2rgb',
      }),
    }),
  );
});

test('claims only persisted Cube workflows and leaves Substitute requests untouched', () => {
  const service = new CubeDirectExecutionService({
    client: {} as CubeExecutionApiClient,
    getGraph: () => null,
    getProximityMatches: () => [],
  });
  const cubeWorkflow = {
    workflow: {
      nodes: [{ properties: { sugarcubes_kind: 'cube' } }],
    },
  };

  expect(service.owns(cubeWorkflow)).toBe(true);
  expect(service.owns({ workflow: { nodes: [] } })).toBe(false);
  expect(
    service.owns({
      ...cubeWorkflow,
      extra_data: { substitute: { execution: true } },
    }),
  ).toBe(false);
});

/** Return a minimal native Cube surface with durable instance identity. */
function cube(instanceId: string, outputs: string[], inputs: string[]): object {
  return {
    properties: { sugarcubes_cube: { instance_id: instanceId } },
    subgraph: {
      outputs: outputs.map((name) => ({ name })),
      inputs: inputs.map((name) => ({ name })),
    },
  };
}
