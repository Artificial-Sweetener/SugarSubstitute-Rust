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
/** Prove native combo reattachment publishes scoped presentation without replacing its control. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import { CubeRelationPresenter } from '../../../frontend/comfyui/ui/relations/presentation/CubeRelationPresenter.js';
import { emptyRelationState } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import type { FieldContract } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';

afterEach(() => document.body.replaceChildren());

test('announces the same combo after native layout replaces its original parent', () => {
  const fields: FieldContract[] = ['a', 'b'].map((id) => ({
    id,
    nodeId: id,
    cubeId: id,
    label: `pack/${id}`,
    name: 'inversion_method',
    semantic: 'inversion_method',
    value: 'euler',
    domain: { kind: 'choice', values: ['euler', 'heun'] },
  }));
  const nodes = fields.map((field) => ({
    id: field.id,
    cubeId: field.id,
    cubeLabel: field.label,
    label: field.label,
    classType: 'Sampler',
    configurationIdentity: 'sampler',
    fields: [field],
    linkable: false,
  }));
  let state = emptyRelationState();
  const relations = new WorkflowRelations({
    inventory: () => ({
      nodes,
      fields: fields.map((contract) => ({ contract, read: () => 'euler', write() {} })),
    }),
    load: () => state,
    save: (next) => {
      state = next;
    },
    transaction: (operation) => operation(),
    refresh() {},
  });
  relations.reconcile();
  const root = document.createElement('article');
  const row = document.createElement('div');
  let parent = document.createElement('div');
  const native = document.createElement('div');
  native.dataset.scFluentComboFrame = '';
  parent.append(native);
  row.append(parent);
  root.append(row);
  document.body.append(root);
  const ready = jest.fn();
  const trigger = document.createElement('button');
  const mount = jest.fn((element: HTMLElement) => {
    element.classList.add('sugarcubes-native-widget');
    element.append(trigger);
    element.addEventListener('sugarcubes:native-presentation-ready', ready);
    return { update: jest.fn(), dispose: jest.fn() };
  });
  const presenter = new CubeRelationPresenter(
    document,
    relations,
    () => [
      {
        nodeId: 'a',
        surface: {
          root,
          header: null,
          rows: new Map([['inversion_method', row]]),
          canvas: null,
          canvasFields: new Map(),
          canvasTitle: false,
        },
      },
    ],
    jest.fn(),
    {
      mount: () => {
        throw new Error('Expected a native choice');
      },
      mountChoice: mount,
    },
  );
  presenter.reconcile();
  const choice = row.querySelector('.sugarcubes-native-widget')!;
  expect(ready).toHaveBeenCalledTimes(1);
  const replacement = document.createElement('div');
  replacement.append(native);
  parent.replaceWith(replacement);
  parent = replacement;
  presenter.reconcile();
  expect(choice.parentElement).toBe(parent);
  expect(choice.querySelector('button')).toBe(trigger);
  expect(mount).toHaveBeenCalledTimes(1);
  expect(ready).toHaveBeenCalledTimes(2);
  presenter.reconcile();
  expect(ready).toHaveBeenCalledTimes(2);
  presenter.dispose();
});
