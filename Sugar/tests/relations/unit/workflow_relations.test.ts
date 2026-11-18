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
import type { SeedStep } from '../../../frontend/comfyui/ui/relations/domain/SharedSeedAdvancement.js';
import type { RelationWorkflowPort } from '../../../frontend/comfyui/ui/relations/application/RelationPorts.js';

/** Provide a real in-memory workflow boundary with serialized state isolation. */
function workflow(name = 'sampler_name', maximum = 100) {
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
              ? { kind: 'number', minimum: 0, maximum, integer: true, step: 1 }
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

test('retires prior bindings and comparison state when undo restores local relation intent', () => {
  const fixture = workflow();
  fixture.relations.reconcile();
  fixture.relations.setOverrideEnabled('sampler_name', false);
  fixture.relations.setFieldValue('a', 'heun');
  const previous = fixture.port.inventory();
  const retiredWrite = jest.fn();
  for (const field of previous.fields) field.write = retiredWrite;
  const read = jest.spyOn(fixture.port, 'inventory').mockReturnValueOnce(previous);
  fixture.relations.reconcile();
  const restored = fixture.state();
  restored.locals.a = 'euler';
  fixture.port.save(restored);
  // The host has replaced widgets; only newly discovered bindings may receive the restored value.
  fixture.values.set('a', 'heun');
  fixture.relations.invalidate();
  fixture.relations.reconcile(false);
  expect(fixture.state().locals.a).toBe('euler');
  expect(fixture.values.get('a')).toBe('euler');
  expect(retiredWrite).not.toHaveBeenCalled();
  expect(read).toHaveBeenCalledTimes(2);
});

test('publishes changed link ownership without refreshing unchanged native values', () => {
  const fixture = workflow();
  fixture.values.set('b', 'euler');
  fixture.relations.reconcile();
  fixture.relations.setOverrideEnabled('sampler_name', false);
  const refresh = jest.spyOn(fixture.port, 'refresh');
  const present = jest.fn();
  fixture.relations.subscribe(present);
  fixture.relations.linkField('b', 'a');
  expect(fixture.relations.snapshot().state.fieldLinks.b).toBe('a');
  expect(fixture.values.get('b')).toBe('euler');
  expect(present).toHaveBeenCalled();
  expect(refresh).not.toHaveBeenCalled();
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

/** Stand in for the host's rule: a fixed roll, and steps that stop at the range's ends. */
const hostStep: SeedStep = (domain, current, mode) =>
  mode === 'randomize'
    ? 50
    : Math.min(
        domain.maximum,
        Math.max(domain.minimum, current + (mode === 'increment' ? domain.step : -domain.step)),
      );

test('a shared seed takes each next value once from the host rule, over the shared range', () => {
  const fixture = workflow('seed');
  fixture.relations.reconcile();
  const step = jest.fn(hostStep);
  fixture.relations.setSeedMode('seed', 'randomize');
  fixture.relations.advanceSeeds(step);
  expect([...fixture.values.values()]).toEqual([50, 50]);
  expect(step).toHaveBeenCalledTimes(1);
  expect(step).toHaveBeenLastCalledWith(
    { kind: 'number', minimum: 0, maximum: 100, integer: true, step: 1 },
    42,
    'randomize',
  );
  fixture.relations.setSeedMode('seed', 'increment');
  fixture.relations.advanceSeeds(step);
  expect([...fixture.values.values()]).toEqual([51, 51]);
  fixture.relations.setOverrideValue('seed', 100);
  fixture.relations.advanceSeeds(step);
  expect([...fixture.values.values()]).toEqual([100, 100]);
  fixture.relations.setSeedMode('seed', 'fixed');
  fixture.relations.advanceSeeds(() => {
    throw new Error('a fixed seed asks for no next value');
  });
  expect([...fixture.values.values()]).toEqual([100, 100]);
});

test('a seed beyond the exact range is rolled but never stepped', () => {
  const fixture = workflow('seed', 2 ** 64);
  const big = Number('17066688399168319613');
  fixture.values.set('a', big);
  fixture.values.set('b', big);
  fixture.relations.reconcile();
  fixture.relations.setSeedMode('seed', 'increment');
  fixture.relations.advanceSeeds(() => {
    throw new Error('one step from this seed cannot be held');
  });
  expect([...fixture.values.values()]).toEqual([big, big]);
  fixture.relations.setSeedMode('seed', 'randomize');
  fixture.relations.advanceSeeds(hostStep);
  expect([...fixture.values.values()]).toEqual([50, 50]);
});

test('held Cubes keep their seed when a run is queued, and only a seed they share is fixed', () => {
  const fixture = workflow('seed');
  fixture.relations.reconcile();
  fixture.relations.setSeedMode('seed', 'randomize');
  fixture.relations.holdSeeds(['elsewhere']);
  expect(fixture.state().overrides.seed?.seedMode).toBe('randomize');
  fixture.relations.holdSeeds(['b']);
  expect(fixture.state().overrides.seed?.seedMode).toBe('fixed');
  fixture.relations.advanceSeeds(() => {
    throw new Error('a held seed asks for no next value');
  });
  expect([...fixture.values.values()]).toEqual([42, 42]);
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
  const step = jest.fn(hostStep);
  fixture.relations.advanceSeeds(step);
  expect([...fixture.values.values()]).toEqual([43, 43]);
  expect(step).toHaveBeenCalledTimes(1);
});

const noScriptState = {
  overrides: [],
  fixedSeedOverrides: [],
  fixedSeedFields: [],
  ownValues: [],
};

test('a script that shares no value leaves every Cube its own', () => {
  const fixture = workflow();
  fixture.relations.reconcile();
  expect([...fixture.values.values()]).toEqual(['euler', 'euler']);
  fixture.relations.adoptScriptRelations(['a', 'b'], noScriptState);
  expect(fixture.state().overrides.sampler_name?.enabled).toBe(false);
  expect([...fixture.values.values()]).toEqual(['euler', 'heun']);
});

test('a value a script gives every Cube is the override, named by its input', () => {
  const fixture = workflow();
  fixture.relations.reconcile();
  fixture.relations.adoptScriptRelations(['a', 'b'], {
    ...noScriptState,
    overrides: [{ key: 'sampler', value: 'heun' }],
  });
  expect(fixture.state().overrides.sampler_name).toMatchObject({ enabled: true, value: 'heun' });
  expect([...fixture.values.values()]).toEqual(['heun', 'heun']);
});

test('a script does not decide the overrides of a workflow that holds other Cubes', () => {
  const fixture = workflow();
  fixture.relations.reconcile();
  fixture.relations.adoptScriptRelations(['b'], noScriptState);
  expect(fixture.state().overrides.sampler_name?.enabled).toBe(true);
});

test('a shared seed is rolled unless the script holds it fixed', () => {
  const rolled = workflow('seed');
  rolled.relations.reconcile();
  rolled.relations.adoptScriptRelations(['a', 'b'], {
    ...noScriptState,
    overrides: [{ key: 'seed', value: 7 }],
  });
  expect(rolled.state().overrides.seed).toEqual({ enabled: true, value: 7, seedMode: 'randomize' });
  const held = workflow('seed');
  held.relations.reconcile();
  held.relations.adoptScriptRelations(['a', 'b'], {
    ...noScriptState,
    overrides: [{ key: 'seed', value: 7 }],
    fixedSeedOverrides: ['seed'],
  });
  expect(held.state().overrides.seed?.seedMode).toBe('fixed');
});

test('a field keeps the value its script records as its own', () => {
  const fixture = workflow();
  fixture.relations.reconcile();
  fixture.relations.adoptScriptRelations(['a', 'b'], {
    ...noScriptState,
    overrides: [{ key: 'sampler_name', value: 'euler' }],
    ownValues: [
      { field: 'b', value: 'heun' },
      { field: 'elsewhere', value: 'heun' },
    ],
  });
  expect(fixture.state().locals.b).toBe('heun');
  expect(fixture.state().locals.elsewhere).toBeUndefined();
  expect(fixture.values.get('b')).toBe('euler');
});
