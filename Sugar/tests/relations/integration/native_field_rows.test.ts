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
/** Characterize canonical row identity before sharing the native adapter with the finder. */
import { afterEach, expect, test } from '@jest/globals';
import { nativeRelationCards } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationCardAdapter.js';
import { registerNativeNodeCard } from '../../../frontend/comfyui/ui/surface/NativeNodeCardBoundary.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

/** Model only the mounted Vue boundary published by Comfy. */
function vnode(
  name: string,
  element: HTMLElement,
  children: readonly UnknownRecord[],
  widget?: string,
): UnknownRecord {
  return {
    type: { __name: name },
    component: {
      subTree: { el: element, children },
      props: widget ? { widget: { name: widget } } : {},
    },
  };
}
afterEach(() => document.body.replaceChildren());
test('resolves canonical widget keys from a local mount independently of visible labels and popups', () => {
  const cell = document.createElement('article');
  cell.className = 'sugarcubes-cube-face__node-card';
  const node = { id: 1, graph: { id: 'face' } };
  registerNativeNodeCard(cell, node, 'cube');
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-cube-face__native-card-mount';
  const card = document.createElement('div');
  card.className = 'lg-node';
  const fields = document.createElement('div');
  const row = document.createElement('div');
  const control = document.createElement('textarea');
  row.append(control);
  fields.append(row);
  card.append(fields);
  mount.append(card);
  cell.append(mount);
  document.body.append(cell);
  Reflect.set(
    mount,
    '_vnode',
    vnode('LGraphNode', card, [
      vnode('NodeWidgets', fields, [vnode('WidgetTextarea', control, [], 'positive_prompt')]),
    ]),
  );
  const surface = nativeRelationCards(document, node, 'cube')[0];
  expect(surface?.rows.get('positive_prompt')).toBe(row);
  expect(surface?.rows.size).toBe(1);
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  document.body.append(popup);
  expect(nativeRelationCards(document, node, 'cube')[0]?.rows.get('positive_prompt')).toBe(row);
});
