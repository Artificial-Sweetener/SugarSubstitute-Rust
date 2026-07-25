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
/** Traverse Comfy's mounted Vue tree through a guarded adapter boundary. */

import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';

export interface ComfyNativeNodeMount {
  component: UnknownRecord;
  appContext: UnknownRecord;
}

/** Resolve Comfy's mounted Vue application context independently of graph contents. */
export function findComfyVueAppContext(root: HTMLElement): UnknownRecord | null {
  const rootRecord: UnknownRecord = isRecord(root) ? root : {};
  const app = isRecord(rootRecord.__vue_app__) ? rootRecord.__vue_app__ : null;
  return isRecord(app?._context) ? app._context : null;
}

/** Locate the actual active LGraphNode component and Comfy application context. */
export function findComfyNativeNodeMount(root: HTMLElement): ComfyNativeNodeMount | null {
  const rootRecord: UnknownRecord = isRecord(root) ? root : {};
  const appContext = findComfyVueAppContext(root);
  if (!appContext) return null;
  const componentVNode = findVueComponent(rootRecord._vnode, 'LGraphNode');
  const component = isRecord(componentVNode?.type) ? componentVNode.type : null;
  return component ? { component, appContext } : null;
}

/** Visit each mounted VNode once and return the requested component VNode. */
export function findVueComponent(value: unknown, name: string): UnknownRecord | null {
  return findVueComponents(value, name)[0] ?? null;
}

/** Visit each mounted VNode once and return every requested component VNode. */
export function findVueComponents(value: unknown, name: string): UnknownRecord[] {
  const pending: unknown[] = [value];
  const visited = new Set<object>();
  const matches: UnknownRecord[] = [];
  while (pending.length > 0) {
    const candidate = pending.pop();
    if (!isRecord(candidate) || visited.has(candidate)) continue;
    visited.add(candidate);
    const type = isRecord(candidate.type) ? candidate.type : null;
    if (type && (type.__name === name || type.name === name)) matches.push(candidate);
    enqueueVueChildren(candidate, pending);
  }
  return matches;
}

/** Add known Vue tree edges without invoking slots or component code. */
function enqueueVueChildren(vnode: UnknownRecord, pending: unknown[]): void {
  const component = isRecord(vnode.component) ? vnode.component : null;
  if (component) {
    pending.push(component.subTree, component.vnode);
  }
  if (Array.isArray(vnode.children)) pending.push(...vnode.children);
  if (Array.isArray(vnode.dynamicChildren)) pending.push(...vnode.dynamicChildren);
  pending.push(vnode.ssContent, vnode.ssFallback);
  const suspense = isRecord(vnode.suspense) ? vnode.suspense : null;
  if (suspense) pending.push(suspense.activeBranch, suspense.pendingBranch);
}
