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
/** Prove workflow-embedded definitions remain selected without catalog support. */

import { jest } from '@jest/globals';

import { CubeDefinitionStore } from '../../../frontend/comfyui/ui/graph/CubeDefinitionStore.js';
import { EmbeddedCubeDefinitionPublisher } from '../../../frontend/comfyui/ui/workflow/EmbeddedCubeDefinitionPublisher.js';

const cubeId = 'Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube';
const cubeVersion = '1.2.0';
const definitionKey = `${cubeId}@${cubeVersion}`;

function embeddedWorkflow(nodeType = 'SimpleSyrup.SimpleLoadCheckpoint') {
  const definitionIdentity = { cube_id: cubeId, cube_version: cubeVersion };
  return {
    nodes: [
      {
        id: 7,
        type: 'definition-sdxl',
        properties: {
          sugarcubes_kind: 'cube',
          sugarcubes_cube: { ...definitionIdentity, instance_id: 'cube-1' },
        },
      },
    ],
    definitions: {
      subgraphs: [
        {
          id: 'definition-sdxl',
          nodes: [{ id: 1, type: nodeType }],
          links: [],
          extra: { sugarcubes_kind: 'cube', sugarcubes_cube: definitionIdentity },
        },
      ],
    },
  };
}

test('publishes an offline embedded definition as non-expiring authority', () => {
  const logger = { warn: jest.fn() };
  const store = new CubeDefinitionStore({ logger });
  const publisher = new EmbeddedCubeDefinitionPublisher(store);

  expect(publisher.publish(embeddedWorkflow())).toBe(1);
  expect(store.ensure({ cubeId, cubeVersion })).toMatchObject({
    authority: 'embedded',
    definitionKey,
    status: 'ready',
    expiresAt: Number.POSITIVE_INFINITY,
  });
  expect(logger.warn).not.toHaveBeenCalled();
});

test('does not let a divergent catalog response shadow embedded content', async () => {
  const api = {
    load: jest.fn(async () => ({ response: { ok: true }, data: { catalog: 'divergent' } })),
    loadRevision: jest.fn(async () => ({ response: { ok: true }, data: { catalog: 'divergent' } })),
  };
  const store = new CubeDefinitionStore({ api });
  const publisher = new EmbeddedCubeDefinitionPublisher(store);
  const workflow = embeddedWorkflow();
  publisher.publish(workflow);

  await store.loadDefinition({ cubeId, cubeVersion });

  expect(api.load).not.toHaveBeenCalled();
  expect(store.getEntry(definitionKey)).toMatchObject({
    authority: 'embedded',
    payload: workflow.definitions.subgraphs[0],
  });
});

test('releases prior workflow authority before a catalog placement', async () => {
  const catalog = { catalog: 'current' };
  const api = {
    load: jest.fn(async () => ({ response: { ok: true }, data: catalog })),
    loadRevision: jest.fn(async () => ({ response: { ok: true }, data: catalog })),
  };
  const store = new CubeDefinitionStore({ api });
  const publisher = new EmbeddedCubeDefinitionPublisher(store);
  publisher.publish(embeddedWorkflow('OldWorkflowNode'));

  publisher.publish({ nodes: [], definitions: { subgraphs: [] } });
  await store.loadDefinition({ cubeId, cubeVersion });

  expect(api.load).toHaveBeenCalledTimes(1);
  expect(store.getEntry(definitionKey)).toMatchObject({
    authority: 'catalog',
    payload: catalog,
  });
});

test('rejects a marked instance whose executable definition is absent', () => {
  const store = new CubeDefinitionStore();
  const publisher = new EmbeddedCubeDefinitionPublisher(store);
  const workflow = embeddedWorkflow();
  workflow.definitions.subgraphs = [];

  expect(() => publisher.publish(workflow)).toThrow(
    "missing embedded definition 'definition-sdxl'",
  );
});

test('recognizes a durable Cube draft without requiring catalog membership', () => {
  const store = new CubeDefinitionStore();
  const publisher = new EmbeddedCubeDefinitionPublisher(store);
  const workflow = embeddedWorkflow();
  workflow.nodes[0]!.properties.sugarcubes_kind = 'cube_draft';
  workflow.definitions.subgraphs[0]!.extra.sugarcubes_kind = 'cube_draft';

  expect(publisher.publish(workflow)).toBe(1);
  expect(store.getEntry(definitionKey)).toMatchObject({ authority: 'embedded' });
});

test('rejects a marked definition with a noncanonical claimed identity', () => {
  const publisher = new EmbeddedCubeDefinitionPublisher(new CubeDefinitionStore());
  const workflow = embeddedWorkflow();
  workflow.definitions.subgraphs[0]!.extra.sugarcubes_cube.cube_id = 'not-a-cube-id';

  expect(() => publisher.publish(workflow)).toThrow('canonical Cube id');
});
