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
/** Prove Substitute's title, link, activation order and shared link width preference on native headers. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import { CubeRelationPresenter } from '../../../frontend/comfyui/ui/relations/presentation/CubeRelationPresenter.js';
import { ensureRelationStyles } from '../../../frontend/comfyui/ui/relations/presentation/RelationStyles.js';
import { emptyRelationState } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import type { FieldContract } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';

afterEach(() => {
  document.body.replaceChildren();
  document.getElementById('sc-relation-styles')?.remove();
});

test('retains native selectors while matching Substitute first-endpoint visibility through reorder and unlink', () => {
  ensureRelationStyles(document);
  const fields: FieldContract[] = ['a', 'b'].map((id) => ({
    id,
    nodeId: id,
    cubeId: id,
    label: `pack/${id}`,
    name: 'variation_strength',
    semantic: 'variation_strength',
    value: 0,
    domain: { kind: 'number', minimum: 0, maximum: 1, step: 0.01, integer: false },
  }));
  let nodes = fields.map((field) => ({
    id: field.nodeId,
    cubeId: field.id,
    cubeLabel: field.label,
    label: `${field.label} · Seed Variation`,
    classType: 'SeedVariation',
    configurationIdentity: 'variation',
    fields: [field],
    linkable: true,
  }));
  const originalOrder = [...nodes];
  let state = emptyRelationState();
  const relations = new WorkflowRelations({
    inventory: () => ({
      nodes,
      fields: fields.map((contract) => ({ contract, read: () => 0, write: () => undefined })),
    }),
    load: () => state,
    save: (next) => {
      state = next;
    },
    transaction: (operation) => operation(),
    refresh: () => undefined,
  });
  relations.reconcile();
  const views = nodes.map((node) => {
    const root = document.createElement('article');
    const header = document.createElement('div');
    const row = document.createElement('div');
    const title = document.createElement('span');
    title.textContent = 'Seed Variation';
    const activation = document.createElement('label');
    activation.className = 'sugarcubes-cube-face__activation';
    row.append(title, activation);
    header.append(row);
    root.append(header);
    document.body.append(root);
    return {
      nodeId: node.id,
      surface: {
        root,
        header,
        rows: new Map([['variation_strength', document.createElement('div')]]),
        canvas: null,
        canvasFields: new Map(),
        canvasTitle: false,
      },
    };
  });
  const mounts = new Map<HTMLElement, ReturnType<typeof jest.fn>>();
  const presenter = new CubeRelationPresenter(document, relations, () => views, jest.fn(), {
    mount: () => {
      throw new Error('Node titlebars must use native choice components');
    },
    mountChoice: (root) => {
      const update = jest.fn();
      mounts.set(root, update);
      return { update, dispose: jest.fn() };
    },
  });
  presenter.reconcile();
  const hiddenRow = views[1]!.surface.rows.get('variation_strength')!;
  const annotations = new MutationObserver(() => undefined);
  annotations.observe(hiddenRow, { attributes: true });
  presenter.reconcile();
  expect(hiddenRow.dataset.scRelationHidden).toBe('true');
  expect(annotations.takeRecords()).toHaveLength(0);
  annotations.disconnect();
  expect(views[0]!.surface.root.querySelector('.sc-node-link')).toBeNull();
  const follower = views[1]!.surface.root.querySelector<HTMLElement>('.sc-node-link')!;
  expect(follower.nextElementSibling).toBe(
    views[1]!.surface.root.querySelector('.sugarcubes-cube-face__activation'),
  );
  expect(follower.previousElementSibling?.textContent).toBe('Seed Variation');
  expect(mounts.get(follower)?.mock.lastCall?.[3]).toEqual(
    expect.arrayContaining(['Independent', '🔗 pack/a', '🔗 pack/b']),
  );
  expect(mounts.get(follower)?.mock.lastCall?.[1]).toEqual([
    { value: '', label: 'Independent', blockers: [] },
    { value: 'a', label: '🔗 pack/a', blockers: [] },
  ]);
  expect(() => relations.linkNode('a', 'b')).toThrow();
  expect(relations.snapshot().state.nodeLinks.b).toBe('a');
  nodes = [...nodes].reverse();
  relations.reconcile();
  presenter.reconcile();
  expect(relations.snapshot().state.nodeLinks.b).toBeNull();
  expect(relations.snapshot().state.nodeLinks.a).toBe('b');
  expect(views[1]!.surface.root.querySelector('.sc-node-link')).toBe(follower);
  expect(follower.isConnected).toBe(true);
  expect(follower.hidden).toBe(true);
  expect(window.getComputedStyle(follower).display).toBe('none');
  expect(hiddenRow.hasAttribute('data-sc-relation-hidden')).toBe(false);
  for (const order of [originalOrder, [...originalOrder].reverse(), originalOrder]) {
    nodes = order;
    relations.reconcile();
    presenter.reconcile();
    expect(views[1]!.surface.root.querySelector('.sc-node-link')).toBe(follower);
    expect(mounts.get(follower)?.mock.lastCall?.[0]).toBe(order[0]?.id === 'b' ? '' : 'a');
    expect(follower.hidden).toBe(order[0]?.id === 'b');
  }
  nodes = [...originalOrder].reverse();
  relations.reconcile();
  presenter.reconcile();
  relations.linkNode('b', null);
  presenter.reconcile();
  expect(hiddenRow.hasAttribute('data-sc-relation-hidden')).toBe(false);
  relations.linkNode('a', null);
  presenter.reconcile();
  const later = views[0]!.surface.root.querySelector<HTMLElement>('.sc-node-link')!;
  expect(mounts.get(later)?.mock.lastCall?.[1]).toEqual([
    { value: '', label: 'Independent', blockers: [] },
    { value: 'b', label: '🔗 pack/b', blockers: [] },
  ]);
  expect(views[1]!.surface.root.querySelector('.sc-node-link')).not.toBeNull();
  nodes = originalOrder;
  relations.reconcile();
  presenter.reconcile();
  expect(later.isConnected).toBe(true);
  expect(views[0]!.surface.root.querySelector('.sc-node-link')).toBe(later);
  expect(later.hidden).toBe(true);
  expect(views[1]!.surface.root.querySelector('.sc-node-link')).not.toBeNull();
  relations.linkNode('b', 'a');
  presenter.reconcile();
  expect(views[0]!.surface.root.querySelector<HTMLElement>('.sc-node-link')?.hidden).toBe(true);
  presenter.dispose();
  expect(document.querySelector('.sc-node-link')).toBeNull();
  expect(document.querySelectorAll('.sugarcubes-cube-face__activation')).toHaveLength(2);
});

test('lists only earlier compatible Cubes in their current workflow order', () => {
  let order = ['a', 'c', 'b', 'd'];
  const fields: FieldContract[] = order.map((id) => ({
    id,
    nodeId: id,
    cubeId: id,
    label: id,
    name: 'value',
    semantic: 'text',
    value: id,
    domain: { kind: 'string' },
  }));
  let state = emptyRelationState();
  state.nodeLinks = Object.fromEntries(order.map((id) => [id, null]));
  const relations = new WorkflowRelations({
    inventory: () => ({
      nodes: order.map((id) => ({
        id,
        cubeId: id,
        cubeLabel: id,
        label: id,
        classType: 'StringConstant',
        configurationIdentity: 'positive',
        promptRole: 'positive' as const,
        fields: fields.filter((field) => field.id === id),
        linkable: true,
      })),
      fields: fields.map((contract) => ({
        contract,
        read: () => contract.value,
        write: () => undefined,
      })),
    }),
    load: () => state,
    save: (next) => {
      state = next;
    },
    transaction: (operation) => operation(),
    refresh: () => undefined,
  });
  const views = fields.map((field) => {
    const root = document.createElement('article');
    const header = document.createElement('header');
    header.append(document.createElement('div'));
    root.append(header);
    document.body.append(root);
    return {
      nodeId: field.id,
      surface: {
        root,
        header,
        rows: new Map<string, HTMLElement>(),
        canvas: null,
        canvasFields: new Map(),
        canvasTitle: false,
      },
    };
  });
  const updates = new Map<string, ReturnType<typeof jest.fn>>();
  const presenter = new CubeRelationPresenter(document, relations, () => views, jest.fn(), {
    mount: () => {
      throw new Error('Use native title choices');
    },
    mountChoice: (_root, label) => {
      const update = jest.fn();
      updates.set(label, update);
      return { update, dispose: jest.fn() };
    },
  });
  for (const next of [order, ['b', 'd', 'a', 'c'], ['d', 'c', 'a', 'b']]) {
    order = next;
    relations.reconcile();
    presenter.reconcile();
    for (const [index, id] of order.entries()) {
      if (index === 0) continue;
      expect(updates.get(`Link ${id}`)?.mock.lastCall?.[1]).toEqual([
        { value: '', label: 'Independent', blockers: [] },
        ...order
          .slice(0, index)
          .map((peer) => ({ value: peer, label: `🔗 ${peer}`, blockers: [] })),
      ]);
    }
  }
  presenter.dispose();
});
