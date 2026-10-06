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
/** Prove automatic override membership and independent cross-Cube clone behavior. */
import { expect, test } from '@jest/globals';
import {
  compatibleNodes,
  canLinkNode,
  discoverOverrides,
  initializeNodeLinks,
  resolveRelations,
  canLinkField,
} from '../../../frontend/comfyui/ui/relations/domain/RelationResolver.js';
import { emptyRelationState } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import type {
  FieldContract,
  NodeContract,
} from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';

/** Provide an eligible field with deterministic stable identity. */
function field(id: string, cubeId: string | null, value = 'euler'): FieldContract {
  return {
    id,
    nodeId: id,
    cubeId,
    label: id,
    name: 'sampler_name',
    semantic: 'sampler_name',
    value,
    domain: { kind: 'choice', values: ['euler', 'heun'] },
  };
}
/** Provide a graph-independent resource patch configuration. */
function node(id: string, cubeId: string): NodeContract {
  return {
    id,
    cubeId,
    cubeLabel: cubeId,
    label: id,
    classType: 'Patch',
    linkable: true,
    fields: [field(id, cubeId)],
  };
}

test('automatically enrolls loose and Cube fields regardless of their selected values', () => {
  const fields = [field('a', 'cube', 'euler'), field('b', null, 'heun')];
  expect(discoverOverrides(fields, emptyRelationState())[0]?.members).toHaveLength(2);
  expect(resolveRelations(fields, [], emptyRelationState()).map((item) => item.value)).toEqual([
    'euler',
    'euler',
  ]);
});

test('links clone configuration only between different Cubes and preserves explicit independence', () => {
  const a = node('a', 'first'),
    b = node('b', 'second'),
    c = node('c', 'first');
  const state = emptyRelationState();
  state.nodeLinks.b = null;
  initializeNodeLinks([a, b, c], state);
  expect(state.nodeLinks.b).toBeNull();
  expect(compatibleNodes(a, c)).toBe(false);
  expect(compatibleNodes(a, { ...b, linkable: false })).toBe(false);
});

test('resolves linked values, retains dormant local values, and gives enabled overrides precedence', () => {
  const nodes = [node('a', 'first'), node('b', 'second')],
    fields = nodes.flatMap((item) => item.fields),
    state = emptyRelationState();
  state.overrides.sampler_name = { enabled: false, value: 'heun', seedMode: 'fixed' };
  state.fieldLinks.b = 'a';
  state.locals.b = 'heun';
  expect(resolveRelations(fields, nodes, state)[1]).toMatchObject({ source: 'a', value: 'euler' });
  state.overrides.sampler_name.enabled = true;
  expect(resolveRelations(fields, nodes, state)[1]).toMatchObject({
    override: 'sampler_name',
    value: 'heun',
  });
  expect(state.fieldLinks.b).toBe('a');
  expect(state.locals.b).toBe('heun');
});

test('rejects cross-workflow field linking and cycles while allowing later Cube masters', () => {
  const fields = [field('a', 'first'), field('b', 'second'), field('loose', null)],
    state = emptyRelationState();
  expect(canLinkField(fields[0]!, fields[1]!, fields, [], state)).toBe(true);
  expect(canLinkField(fields[0]!, fields[2]!, fields, [], state)).toBe(false);
  state.fieldLinks.b = 'a';
  expect(canLinkField(fields[0]!, fields[1]!, fields, [], state)).toBe(false);
});

test('malformed persisted cycles resolve independently instead of recursing', () => {
  const fields = [field('a', 'first'), field('b', 'second', 'heun')],
    state = emptyRelationState();
  state.overrides.sampler_name = { enabled: false, value: 'euler', seedMode: 'fixed' };
  state.fieldLinks = { a: 'b', b: 'a' };
  expect(resolveRelations(fields, [], state).map((item) => item.value)).toEqual(['euler', 'heun']);
});

test('rejects clone groups whose pairwise overlap has no value common to the whole group', () => {
  const a = node('a', 'first'),
    b = node('b', 'second'),
    c = node('c', 'third');
  const withChoices = (target: NodeContract, values: string[]): NodeContract => ({
    ...target,
    fields: target.fields.map((field) => ({ ...field, domain: { kind: 'choice', values } })),
  });
  const nodes = [
    withChoices(a, ['euler', 'heun']),
    withChoices(b, ['euler', 'dpm']),
    withChoices(c, ['heun', 'dpm']),
  ];
  const state = emptyRelationState();
  state.nodeLinks.b = 'a';
  expect(compatibleNodes(nodes[2]!, nodes[0]!)).toBe(true);
  expect(canLinkNode(nodes[2]!, nodes[0]!, nodes, state)).toBe(false);
  initializeNodeLinks(nodes, state);
  expect(state.nodeLinks.c).toBeUndefined();
});
