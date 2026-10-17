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
/** Resolve canonical native widget rows through their own mounted Vue boundary. */
import { isRecord } from '../types/common.js';
import { findVueComponents, findMountedNodeVNode } from './ComfyVueTree.js';
import { NATIVE_FIELD_COMPONENTS } from './appearance/NativeFieldPresentation.js';

/** Share identified row lookup between native appearance consumers without graph candidate discovery. */
export function nativeNodeFieldRows(root: HTMLElement): ReadonlyMap<string, HTMLElement> {
  const document = root.ownerDocument;
  const card = root.matches('.lg-node') ? root : root.querySelector<HTMLElement>('.lg-node');
  const rows = new Map<string, HTMLElement>();
  const vnodeRoot =
    root.querySelector<HTMLElement>('.sugarcubes-cube-face__native-card-mount') ?? card;
  const localVNode: unknown = vnodeRoot ? Reflect.get(vnodeRoot, '_vnode') : null;
  const appRoot = document.getElementById('vue-app');
  const appVNode: unknown = appRoot ? Reflect.get(appRoot, '_vnode') : null;
  const vnode =
    localVNode ??
    (card ? findMountedNodeVNode(card) : null) ??
    (card
      ? findVueComponents(appVNode, 'LGraphNode').find((candidate) => {
          const component = isRecord(candidate.component) ? candidate.component : {};
          const subtree = isRecord(component.subTree) ? component.subTree : {};
          return subtree.el === card;
        })
      : undefined);
  if (vnode)
    for (const container of findVueComponents(vnode, 'NodeWidgets')) {
      const component = isRecord(container.component) ? container.component : {},
        tree = isRecord(component.subTree) ? component.subTree : {};
      if (!(tree.el instanceof HTMLElement)) continue;
      for (const child of findVueComponents(container, [
        ...NATIVE_FIELD_COMPONENTS,
        'WidgetToggle',
      ])) {
        const instance = isRecord(child.component) ? child.component : {},
          props = isRecord(instance.props) ? instance.props : {},
          widget = isRecord(props.widget) ? props.widget : {},
          subtree = isRecord(instance.subTree) ? instance.subTree : {};
        if (!(subtree.el instanceof HTMLElement) || typeof widget.name !== 'string') continue;
        let row = subtree.el;
        while (row.parentElement && row.parentElement !== tree.el) row = row.parentElement;
        if (row.parentElement === tree.el) rows.set(widget.name, row);
      }
    }
  if (card)
    for (const row of card.querySelectorAll<HTMLElement>('[data-sc-style-row]')) {
      if (row.closest('.lg-node') !== card) continue;
      const label = row
        .querySelector<HTMLElement>('[data-testid="widget-layout-field-label"]')
        ?.textContent?.trim();
      const control = row.querySelector<HTMLElement>('[role="combobox"][aria-label], [aria-label]');
      const name = control?.getAttribute('aria-label');
      if (name) rows.set(name, row);
      else if (label) rows.set(label, row);
    }
  return rows;
}
