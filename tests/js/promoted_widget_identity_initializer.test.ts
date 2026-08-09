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

import { jest } from '@jest/globals';
import { initializePromotedWidgetIdentity } from '../../frontend/comfyui/ui/cube/node/PromotedWidgetIdentityInitializer.js';

describe('promoted widget identity initialization', () => {
  test('reconfigures a nested host after its durable node id is assigned', () => {
    const registeredWidgetIds: string[] = [];
    const node = {
      id: 'durable-node-id',
      isSubgraphNode: () => true,
      configure: jest.fn(function (this: { id: string }) {
        registeredWidgetIds.push(`root:${this.id}:model_name`);
      }),
    };

    initializePromotedWidgetIdentity(node);

    expect(node.configure).toHaveBeenCalledWith({});
    expect(registeredWidgetIds).toEqual(['root:durable-node-id:model_name']);
  });

  test('leaves ordinary nodes untouched', () => {
    const configure = jest.fn();

    initializePromotedWidgetIdentity({ id: 42, isSubgraphNode: () => false, configure });

    expect(configure).not.toHaveBeenCalled();
  });

  test('fails closed when a subgraph host cannot rebuild promoted widgets', () => {
    expect(() =>
      initializePromotedWidgetIdentity({ id: 'nested', isSubgraphNode: () => true }),
    ).toThrow("Subgraph node 'nested' cannot initialize its widgets");
  });
});
