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
/** Prove eligible native fields, seed leases, persistence, and queued values through real adapters. */
import { expect, test } from '@jest/globals';
import { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import { NativeRelationWorkflowPort } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationWorkflowPort.js';
import { NativeRelationExecution } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationExecution.js';
import { NativeSharedSeedModes } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeSharedSeedModes.js';
import { NativeRelationVisibility } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationVisibility.js';
import {
  cubeFaceRevealableWidgets,
  cubeFacePresentedWidgets,
} from '../../../frontend/comfyui/ui/surface/CubeFaceWidgetPolicy.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

/** Create native seed controls with independent local generation behavior. */
function seed(id: number, value: number): ComfyNode {
  const mode = {
    name: 'control_after_generate',
    type: 'combo',
    value: 'increment',
    options: { values: ['fixed', 'randomize', 'increment', 'decrement'] },
  };
  return {
    id,
    type: 'Seed',
    widgets: [
      {
        name: 'seed',
        type: 'number',
        value,
        options: { min: 0, max: 100, precision: 0 },
        linkedWidgets: [mode],
      },
      mode,
    ],
  };
}

test('queues one shared seed and current workflow metadata while preserving real upstream connections', () => {
  const first = seed(1, 10),
    second = seed(2, 20),
    connected = seed(3, 30);
  connected.inputs = [{ name: 'seed', widget: { name: 'seed' }, link: 0 }];
  const graph: ComfyGraph = { _nodes: [first, second, connected], extra: {} };
  graph.serialize = () => ({
    extra: graph.extra,
    nodes: graph._nodes?.map((node) => ({
      id: node.id,
      widgets_values: node.widgets?.map((widget) => widget.value),
    })),
  });
  const port = new NativeRelationWorkflowPort(
    () => graph,
    document,
    console,
    () => null,
  );
  const relations = new WorkflowRelations(port),
    modes = new NativeSharedSeedModes(),
    visibility = new NativeRelationVisibility();
  relations.reconcile();
  modes.reconcile(relations.snapshot(), port.inventorySource);
  port.save(relations.snapshot().state);
  visibility.reconcile(relations.snapshot(), port.inventorySource);
  expect(cubeFaceRevealableWidgets(first)).toHaveLength(2);
  expect(cubeFacePresentedWidgets(first)).toHaveLength(0);
  expect(first.widgets?.[1]?.value).toBe('fixed');
  expect(connected.widgets?.[1]?.value).toBe('increment');
  const execution = new NativeRelationExecution(relations, port.inventorySource, console, () =>
    graph.serialize?.(),
  );
  const payload = {
    output: {
      '1': { inputs: { seed: 10 } },
      '2': { inputs: { seed: 20 } },
      '3': { inputs: { seed: ['99', 0] } },
    },
    workflow: {},
  };
  const result = execution.prepare(payload);
  expect(result).toMatchObject({
    output: {
      '1': { inputs: { seed: 11 } },
      '2': { inputs: { seed: 11 } },
      '3': { inputs: { seed: ['99', 0] } },
    },
    workflow: { extra: { sugarcubes_relations: { overrides: { seed: { value: 11 } } } } },
  });
  expect(payload.output['2'].inputs.seed).toBe(20);
  first.inputs = [{ name: 'seed', widget: { name: 'seed' }, link: 8 }];
  modes.reconcile(relations.reconcile(), port.inventorySource);
  visibility.reconcile(relations.snapshot(), port.inventorySource);
  expect(first.widgets?.[1]?.value).toBe('increment');
  expect(relations.snapshot().fields).toHaveLength(1);
  relations.setOverrideEnabled('seed', false);
  modes.reconcile(relations.snapshot(), port.inventorySource);
  visibility.reconcile(relations.snapshot(), port.inventorySource);
  expect(second.widgets?.[1]?.value).toBe('increment');
  expect(cubeFacePresentedWidgets(second)).toHaveLength(2);
  modes.dispose();
  visibility.dispose();
});

test('serializes exposed facade values into their exact leaves without enrolling buried fields', () => {
  const leaf = seed(1, 10),
    hidden = seed(2, 77);
  leaf.inputs = [{ name: 'seed', widget: { name: 'seed' } }];
  const facade: ComfyNode = {
    id: 3,
    type: 'Subgraph',
    isSubgraphNode: () => true,
    subgraph: { _nodes: [leaf, hidden] },
    inputs: [{ name: 'publicSeed', widget: { name: 'publicSeed' } }],
    widgets: [
      {
        name: 'publicSeed',
        type: 'number',
        value: 42,
        options: { min: 0, max: 100, precision: 0 },
      },
    ],
    resolveSubgraphInputLinks: () => [
      { inputNode: leaf, input: leaf.inputs?.[0], link: { target_slot: 0 } },
    ],
  };
  const graph: ComfyGraph = { _nodes: [facade], extra: {} };
  const port = new NativeRelationWorkflowPort(
    () => graph,
    document,
    console,
    () => null,
  );
  const relations = new WorkflowRelations(port);
  relations.reconcile();
  relations.setOverrideValue('seed', 50);
  const execution = new NativeRelationExecution(relations, port.inventorySource, console, () => ({
    leafValue: leaf.widgets?.[0]?.value,
  }));
  expect(execution.prepare({ output: {} })).toMatchObject({ workflow: { leafValue: 50 } });
  expect(hidden.widgets?.[0]?.value).toBe(77);
  facade.inputs![0]!.link = 5;
  leaf.widgets![0]!.value = 15;
  execution.prepare({ output: {} });
  expect(leaf.widgets?.[0]?.value).toBe(15);
});
