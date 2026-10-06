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
/** Prove shared edits, persistence, seed advancement, and master removal through typed ports. */
import { expect, jest, test } from '@jest/globals';
import { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import { parseRelationState } from '../../../frontend/comfyui/ui/relations/infrastructure/RelationWorkflowMetadata.js';
import { emptyRelationState } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import type {
  FieldContract,
  RelationState,
} from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import type { RelationWorkflowPort } from '../../../frontend/comfyui/ui/relations/application/RelationPorts.js';

/** Provide a real in-memory workflow boundary with serialized state isolation. */
function workflow(name = 'sampler_name') {
  let state = emptyRelationState();
  const values = new Map<string, string | number | boolean>([
    ['a', name === 'seed' ? 42 : 'euler'],
    ['b', name === 'seed' ? 19 : 'heun'],
  ]);
  let transactions = 0;
  const port: RelationWorkflowPort = {
    inventory: () => ({
      fields: [...values].map(([id, value]) => {
        const contract: FieldContract = {
          id,
          nodeId: id,
          cubeId: id,
          label: id,
          name,
          semantic: name,
          value,
          domain:
            name === 'seed'
              ? { kind: 'number', minimum: 0, maximum: 100, integer: true, step: 1 }
              : { kind: 'choice', values: ['euler', 'heun'] },
        };
        return { contract, read: () => values.get(id)!, write: (next) => values.set(id, next) };
      }),
      nodes: [],
    }),
    load: () => parseRelationState(state),
    save: (next: RelationState) => {
      state = parseRelationState(next);
    },
    transaction: (operation) => {
      transactions++;
      operation();
    },
    refresh: () => undefined,
  };
  return {
    relations: new WorkflowRelations(port),
    values,
    state: () => state,
    transactions: () => transactions,
    port,
  };
}

test('value edits reuse bindings and refresh only the affected relation members', () => {
  const fixture = workflow();
  fixture.relations.reconcile();
  const inventory = jest.spyOn(fixture.port, 'inventory'),
    refresh = jest.spyOn(fixture.port, 'refresh');
  fixture.relations.setOverrideValue('sampler_name', 'heun');
  expect(inventory).not.toHaveBeenCalled();
  expect(refresh).toHaveBeenLastCalledWith(['a', 'b']);
  refresh.mockClear();
  fixture.relations.reconcile(false);
  expect(refresh).not.toHaveBeenCalled();
  expect(inventory).not.toHaveBeenCalled();
});

test('applies automatic globals and edits one shared value through native transaction boundaries', () => {
  const fixture = workflow();
  fixture.relations.reconcile();
  expect([...fixture.values.values()]).toEqual(['euler', 'euler']);
  fixture.relations.setOverrideValue('sampler_name', 'heun');
  expect([...fixture.values.values()]).toEqual(['heun', 'heun']);
  expect(fixture.transactions()).toBeGreaterThan(0);
  expect(() => fixture.relations.setOverrideValue('sampler_name', 'custom')).toThrow();
  expect([...fixture.values.values()]).toEqual(['heun', 'heun']);
});

test('retains links under globals and restores follower independence deliberately', () => {
  const fixture = workflow();
  fixture.relations.reconcile();
  fixture.relations.linkField('b', 'a');
  fixture.relations.setOverrideEnabled('sampler_name', false);
  expect([...fixture.values.values()]).toEqual(['euler', 'euler']);
  fixture.relations.setFieldValue('a', 'heun');
  expect([...fixture.values.values()]).toEqual(['heun', 'heun']);
  fixture.relations.linkField('b', null);
  fixture.relations.setFieldValue('a', 'euler');
  expect([...fixture.values.values()]).toEqual(['euler', 'heun']);
});

test('randomizes and increments once per shared seed, including wraparound', () => {
  const fixture = workflow('seed');
  fixture.relations.reconcile();
  fixture.relations.setSeedMode('seed', 'randomize');
  fixture.relations.advanceSeeds(() => 0.5);
  expect([...fixture.values.values()]).toEqual([50, 50]);
  fixture.relations.setSeedMode('seed', 'increment');
  fixture.relations.advanceSeeds(() => 0);
  expect([...fixture.values.values()]).toEqual([51, 51]);
  fixture.relations.setOverrideValue('seed', 100);
  fixture.relations.advanceSeeds(() => 0);
  expect([...fixture.values.values()]).toEqual([0, 0]);
});

test('removes connected or deleted fields from membership without continuing to write them', () => {
  const fixture = workflow();
  fixture.relations.reconcile();
  fixture.values.delete('b');
  fixture.relations.reconcile();
  expect(fixture.relations.snapshot().groups[0]?.members).toHaveLength(1);
  expect(fixture.state().locals.b).toBeUndefined();
});

test('advances one linked seed master once after its global override is disabled', () => {
  const fixture = workflow('seed');
  fixture.relations.reconcile();
  fixture.relations.setOverrideEnabled('seed', false);
  fixture.relations.linkField('b', 'a');
  fixture.state().localSeedModes.a = 'increment';
  fixture.relations.advanceSeeds(() => {
    throw new Error('increment does not sample randomness');
  });
  expect([...fixture.values.values()]).toEqual([43, 43]);
});
