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
/** Prove override ownership hides empty cards while field discovery and restoration remain stable. */
import { jest } from '@jest/globals';
import { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import { NativeRelationWorkflowPort } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationWorkflowPort.js';
import { NativeRelationVisibility } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationVisibility.js';
import { projectNodeCards } from '../../../frontend/comfyui/ui/surface/NodeCardProjection.js';
import { projectWorkflowMasonry } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryProjection.js';
import { createDefaultCubeSurfaceState } from '../../../frontend/comfyui/ui/surface/CubeSurfaceState.js';
import { buildCubeSurfacePresentationSignature } from '../../../frontend/comfyui/ui/surface/CubeSurfaceSignatures.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { NativeGraphNode } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';

/** Retain actual widget, relation and projection owners while replacing only the native graph boundary. */
function fixture() {
  const style: NativeGraphNode = {
    id: 'style',
    type: 'SimpleSyrup.PromptEncodeStyle',
    title: 'Prompt Encode Style',
    mode: 0,
    pos: [0, 0],
    size: [320, 86],
    properties: { sugarcubes_symbol: 'prompt_encode_style' },
    inputs: [],
    outputs: [{ name: 'encode_style', type: 'STRING' }],
    widgets: [
      {
        name: 'encode_style',
        type: 'combo',
        value: 'standard',
        options: { values: ['standard', 'scheduled'] },
      },
    ],
    connect() {},
  };
  const cube: CubeNode = {
    id: 1,
    type: 'Cube',
    title: 'Inpaint',
    pos: [0, 0],
    size: [400, 600],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'inpaint', cube_id: 'SDXL/Inpaint.cube' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    widgets: [],
    connect() {},
    serialize: () => ({}),
    isSubgraphNode: () => true,
    subgraph: {
      id: 'inpaint-graph',
      name: 'Inpaint',
      _nodes: [style],
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      addInput() {
        return { connect() {} };
      },
      addOutput() {
        return { connect() {} };
      },
      configure() {},
    },
  };
  const graph: ComfyGraph = {
    _nodes: [cube],
    extra: {},
    beforeChange: jest.fn(),
    afterChange: jest.fn(),
  };
  const port = new NativeRelationWorkflowPort(
      () => graph,
      document,
      console,
      () => null,
    ),
    relations = new WorkflowRelations(port),
    visibility = new NativeRelationVisibility();
  /** Apply visibility after each application-owned inventory reconciliation. */
  const reconcile = () => visibility.reconcile(relations.reconcile(), port.inventorySource);
  /** Read the exact policy consumed by Cube faces and root workflow masonry. */
  const card = () =>
    projectNodeCards([style], createDefaultCubeSurfaceState(), cube.subgraph, cube)[0];
  return { style, cube, graph, port, relations, visibility, reconcile, card };
}

test('keeps the last overridden field discoverable while hiding its empty card and restores it on disable and undo', () => {
  const f = fixture(),
    before = buildCubeSurfacePresentationSignature(f.cube);
  expect(f.card()?.visible).toBe(true);
  f.reconcile();
  expect(f.card()?.visible).toBe(false);
  expect(projectWorkflowMasonry(f.graph, [f.cube])[0]?.cards[0]?.visible).toBe(false);
  expect(buildCubeSurfacePresentationSignature(f.cube)).not.toBe(before);
  const field = f.relations.snapshot().fields[0];
  expect(field?.name).toBe('encode_style');
  for (let n = 0; n < 4; n++) {
    f.reconcile();
    expect(f.relations.snapshot().fields.map((candidate) => candidate.id)).toEqual([field?.id]);
    expect(f.relations.snapshot().groups[0]?.enabled).toBe(true);
    expect(f.card()?.visible).toBe(false);
  }
  const overridden = f.port.load();
  f.relations.setOverrideEnabled('encode_style', false);
  f.reconcile();
  expect(f.card()?.visible).toBe(true);
  expect(buildCubeSurfacePresentationSignature(f.cube)).toBe(before);
  f.port.save(overridden);
  f.reconcile();
  expect(f.card()?.visible).toBe(false);
  f.relations.setOverrideEnabled('encode_style', false);
  f.reconcile();
  expect(f.card()?.visible).toBe(true);
  expect(f.style.widgets?.[0]?.value).toBe('standard');
  f.visibility.dispose();
});

test('keeps a partially overridden card visible until its final local row moves out', () => {
  const f = fixture();
  f.style.widgets?.push({
    name: 'strength',
    type: 'number',
    value: 1,
    options: { min: 0, max: 1 },
  });
  f.reconcile();
  expect(f.card()?.visible).toBe(true);
  expect(f.relations.snapshot().fields).toHaveLength(2);
  f.visibility.dispose();
  expect(f.card()?.visible).toBe(true);
});
