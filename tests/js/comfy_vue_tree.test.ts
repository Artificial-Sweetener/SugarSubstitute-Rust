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
/** Characterize native component discovery from Comfy's mounted Vue tree. */

import { describe, expect, test } from '@jest/globals';

import {
  findComfyNativeNodeMount,
  findComfyVueAppContext,
} from '../../frontend/comfyui/ui/surface/ComfyVueTree.js';

describe('findComfyNativeNodeMount', () => {
  test('returns the actual LGraphNode component and existing app context', () => {
    const component = { __name: 'LGraphNode', setup: () => undefined };
    const appContext = { provides: { pinia: 'real-store' } };
    const root = document.createElement('div') as HTMLDivElement & {
      __vue_app__?: unknown;
      _vnode?: unknown;
    };
    root.__vue_app__ = { _context: appContext };
    root._vnode = {
      component: {
        subTree: {
          children: [
            {
              component: {
                subTree: {
                  type: component,
                  props: { nodeData: { id: 'node-1' } },
                },
              },
            },
          ],
        },
      },
    };

    expect(findComfyNativeNodeMount(root)).toEqual({ component, appContext });
  });

  test('returns null until Nodes 2.0 has mounted a native node component', () => {
    const root = document.createElement('div') as HTMLDivElement & {
      __vue_app__?: unknown;
      _vnode?: unknown;
    };
    root.__vue_app__ = { _context: {} };
    root._vnode = { type: { __name: 'GraphView' } };

    expect(findComfyNativeNodeMount(root)).toBeNull();
    expect(findComfyVueAppContext(root)).toEqual({});
  });
});
