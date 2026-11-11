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
/** Prove dormant configuration, relation validity and promotion across workflow transitions. */
import { expect, test } from '@jest/globals';
import { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import { emptyRelationState } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import type {
  FieldContract,
  NodeContract,
} from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import { parseRelationState } from '../../../frontend/comfyui/ui/relations/infrastructure/RelationWorkflowMetadata.js';

/** Keep real editable values separate from durable dormant values and inventory ordering. */
function fixture(choice = false) {
  let order = ['a', 'b', 'c'];
  let state = emptyRelationState();
  const values = new Map(
    order.map((id) => [id, choice ? (id === 'a' ? 'euler' : 'heun') : `${id} local`]),
  );
  const modes = new Map(order.map((id) => [id, id === 'b' ? 4 : 0]));
  const excluded = new Set<string>();
  const fields = (): FieldContract[] =>
    order.map((id) => ({
      id,
      nodeId: id,
      cubeId: id,
      label: id,
      name: choice ? 'algorithm' : 'text',
      semantic: choice ? 'algorithm' : 'text',
      value: values.get(id)!,
      domain: choice ? { kind: 'choice', values: ['euler', 'heun'] } : { kind: 'string' },
      linkable: !excluded.has(id),
    }));
  const relations = new WorkflowRelations({
    inventory: () => {
      const contracts = fields();
      const nodes: NodeContract[] = contracts.map((field) => ({
        id: field.id,
        cubeId: field.id,
        cubeLabel: field.id,
        label: 'Positive Prompt',
        classType: 'StringConstant',
        configurationIdentity: 'positive',
        promptRole: 'positive',
        fields: [field],
        linkable: !choice && !excluded.has(field.id),
        mode: modes.get(field.id)!,
      }));
      return {
        nodes,
        fields: contracts.map((contract) => ({
          contract,
          read: () => values.get(contract.id)!,
          write: (value) => {
            values.set(contract.id, String(value));
          },
        })),
        editableNodes: nodes.map((node) => ({
          id: node.id,
          readMode: () => modes.get(node.id)!,
          writeMode: (mode) => {
            modes.set(node.id, mode);
          },
        })),
      };
    },
    load: () => parseRelationState(state),
    save: (next) => {
      state = parseRelationState(next);
    },
    transaction: (operation) => operation(),
    refresh: () => undefined,
  });
  return {
    relations,
    values,
    modes,
    excluded,
    reorder: (ids: string[]) => {
      order = ids;
      return relations.reconcile();
    },
    capture: () => ({
      state: parseRelationState(state),
      values: [...values],
      modes: [...modes],
      order: [...order],
    }),
    restore: (saved: {
      state: typeof state;
      values: [string, string][];
      modes: [string, number][];
      order: string[];
    }) => {
      state = parseRelationState(saved.state);
      order = [...saved.order];
      values.clear();
      for (const [id, value] of saved.values) values.set(id, value);
      modes.clear();
      for (const [id, mode] of saved.modes) modes.set(id, mode);
      return relations.reconcile();
    },
  };
}

test('the first linked prompt owns visible shared contents after reorder', () => {
  const f = fixture();
  f.relations.reconcile();
  const snapshot = f.reorder(['b', 'a', 'c']);
  expect(snapshot.state.nodeLinks).toEqual({ a: 'b', b: null, c: 'b' });
  expect(snapshot.resolved.find((field) => field.field.id === 'b')?.source).toBeNull();
  expect(f.values.get('b')).toBe('a local');
});

test('repeated reorder promotes the first group member while preserving shared contents', () => {
  const f = fixture();
  f.relations.reconcile();
  for (const ids of [
    ['c', 'b', 'a'],
    ['b', 'a', 'c'],
    ['a', 'c', 'b'],
    ['c', 'a', 'b'],
  ]) {
    const snapshot = f.reorder(ids);
    expect(snapshot.state.nodeLinks[ids[0]!]).toBeNull();
    for (const follower of ids.slice(1)) expect(snapshot.state.nodeLinks[follower]).toBe(ids[0]);
    expect([...f.values.values()]).toEqual(['a local', 'a local', 'a local']);
  }
});

test('unlinking an unpromoted follower reveals its dormant text and activation', () => {
  const f = fixture();
  f.relations.reconcile();
  f.reorder(['c', 'b', 'a']);
  f.relations.linkNode('b', null);
  expect(f.values.get('b')).toBe('b local');
  expect(f.modes.get('b')).toBe(4);
  f.reorder(['a', 'b', 'c']);
  expect(f.relations.snapshot().state.nodeLinks.b).toBeNull();
  expect(f.values.get('b')).toBe('b local');
});

test('a surviving source that becomes ineligible unlinks followers and restores their contents', () => {
  const f = fixture();
  f.relations.reconcile();
  f.excluded.add('a');
  const snapshot = f.reorder(['b', 'c', 'a']);
  expect(snapshot.state.nodeLinks).toMatchObject({ b: null, c: null });
  expect(f.values.get('b')).toBe('b local');
  expect(f.values.get('c')).toBe('c local');
  expect(f.modes.get('b')).toBe(4);
  f.excluded.clear();
  f.reorder(['a', 'b', 'c']);
  expect(f.relations.snapshot().state.nodeLinks.b).toBeNull();
});

test('unlinking a follower leaves its peers together through master promotion', () => {
  const f = fixture();
  f.relations.reconcile();
  f.relations.linkNode('b', null);
  const snapshot = f.reorder(['c', 'b', 'a']);
  expect(snapshot.state.nodeLinks).toMatchObject({ a: 'c', b: null, c: null });
  expect(f.values.get('c')).toBe('a local');
  expect(f.values.get('b')).toBe('b local');
  expect(snapshot.state.locals.b).toBe('b local');
});

test('field links preserve selected sources across reorder but release an ineligible source', () => {
  const f = fixture(true);
  f.relations.reconcile();
  f.relations.linkField('b', 'a');
  expect(f.values.get('b')).toBe('euler');
  expect(f.reorder(['b', 'c', 'a']).state.fieldLinks.b).toBe('a');
  f.excluded.add('a');
  f.relations.reconcile();
  expect(f.relations.snapshot().state.fieldLinks.b).toBeNull();
  expect(f.values.get('b')).toBe('heun');
});

test('removed master promotes the first surviving member in current order with effective contents', () => {
  const f = fixture();
  f.relations.reconcile();
  const snapshot = f.reorder(['c', 'b']);
  expect(snapshot.state.nodeLinks).toEqual({ c: null, b: 'c' });
  expect(f.values.get('c')).toBe('a local');
  expect(snapshot.state.locals.c).toBe('a local');
  expect(snapshot.state.locals.b).toBe('b local');
  f.relations.linkNode('b', null);
  expect(f.values.get('b')).toBe('b local');
});

test('restored workflow snapshots retain dormant values through repeated undo and redo', () => {
  const f = fixture();
  f.relations.reconcile();
  const linked = f.capture();
  f.reorder(['b', 'c', 'a']);
  f.relations.linkNode('c', null);
  const independent = f.capture();
  for (let cycle = 0; cycle < 3; cycle++) {
    expect(f.restore(linked).state.nodeLinks.b).toBe('a');
    expect(f.values.get('b')).toBe('a local');
    expect(f.restore(independent).state.nodeLinks.c).toBeNull();
    expect(f.values.get('c')).toBe('c local');
    expect(f.modes.get('c')).toBe(0);
  }
});
