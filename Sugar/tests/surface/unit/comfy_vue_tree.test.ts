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
  findVueComponent,
  findComfyVueAppContext,
  findVueComponents,
} from '../../../frontend/comfyui/ui/surface/ComfyVueTree.js';

describe('Comfy Vue tree boundaries', () => {
  test('stops singular discovery at the first match and batches component types in one walk', () => {
    let visits = 0;
    const tail = {
      get type() {
        visits += 1;
        return { __name: 'WidgetTextarea' };
      },
    };
    const first = { type: { __name: 'WidgetSelect' } };
    const tree = { children: [tail, first] };
    expect(findVueComponent(tree, 'WidgetSelect')).toBe(first);
    expect(visits).toBe(0);
    expect(findVueComponents(tree, ['WidgetSelect', 'WidgetTextarea'])).toEqual([first, tail]);
    expect(visits).toBe(1);
  });
  test('discovers native VNodes for annotations independently of the application context', () => {
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

    expect(findVueComponent(root._vnode, 'LGraphNode')?.type).toBe(component);
    expect(findComfyVueAppContext(root)).toBe(appContext);
  });

  test('returns null until Nodes 2.0 has mounted a native node component', () => {
    const root = document.createElement('div') as HTMLDivElement & {
      __vue_app__?: unknown;
      _vnode?: unknown;
    };
    root.__vue_app__ = { _context: {} };
    root._vnode = { type: { __name: 'GraphView' } };

    expect(findVueComponent(root._vnode, 'LGraphNode')).toBeNull();
    expect(findComfyVueAppContext(root)).toEqual({});
  });
});
