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
/** Resolve a native node through its own component ancestry instead of walking the application tree. */
export function findMountedNodeVNode(root) {
    let candidate = Reflect.get(root, '__vueParentComponent');
    const visited = new Set();
    while (isRecord(candidate) && !visited.has(candidate)) {
        visited.add(candidate);
        const vnode = isRecord(candidate.vnode) ? candidate.vnode : null;
        const type = isRecord(vnode?.type) ? vnode.type : null;
        if (type?.__name === 'LGraphNode' || type?.name === 'LGraphNode')
            return vnode;
        candidate = candidate.parent;
    }
    return null;
}
/** Resolve exact production card mounts when Vue omits development-only component ancestry. */
export function findNativeNodeVNode(root) {
    const direct = findMountedNodeVNode(root);
    if (direct)
        return direct;
    const mount = root.closest('.sugarcubes-native-node-card') ?? root.ownerDocument.getElementById('vue-app');
    const record = isRecord(mount) ? mount : {};
    return (findVueComponents(record._vnode, 'LGraphNode').find((vnode) => resolveVueElement(vnode) === root) ?? null);
}
/** Narrow a mounted component's actual element across native and wrapped Vue VNodes. */
export function resolveVueElement(vnode) {
    const component = isRecord(vnode.component) ? vnode.component : {};
    const tree = isRecord(component.subTree) ? component.subTree : {};
    return tree.el instanceof HTMLElement
        ? tree.el
        : vnode.el instanceof HTMLElement
            ? vnode.el
            : null;
}
/** Resolve Comfy's mounted Vue application context independently of graph contents. */
export function findComfyVueAppContext(root) {
    const rootRecord = isRecord(root) ? root : {};
    const app = isRecord(rootRecord.__vue_app__) ? rootRecord.__vue_app__ : null;
    return isRecord(app?._context) ? app._context : null;
}
/** Visit each mounted VNode once and return the requested component VNode. */
export function findVueComponent(value, name) {
    const desired = new Set([name]);
    for (const candidate of mountedVueNodes(value))
        if (hasVueComponentName(candidate, desired))
            return candidate;
    return null;
}
/** Visit each mounted VNode once and return every requested component VNode. */
export function findVueComponents(value, names) {
    const desired = new Set(typeof names === 'string' ? [names] : names);
    const matches = [];
    for (const candidate of mountedVueNodes(value))
        if (hasVueComponentName(candidate, desired))
            matches.push(candidate);
    return matches;
}
/** Read the component's published name without invoking its render code. */
function hasVueComponentName(candidate, desired) {
    const value = candidate.type;
    if (!isRecord(value))
        return false;
    return ((typeof value.__name === 'string' && desired.has(value.__name)) ||
        (typeof value.name === 'string' && desired.has(value.name)));
}
/** Share one cycle-safe traversal between early lookup and batched type discovery. */
function* mountedVueNodes(value) {
    const pending = [value];
    const visited = new Set();
    while (pending.length > 0) {
        const candidate = pending.pop();
        if (!isRecord(candidate) || visited.has(candidate))
            continue;
        visited.add(candidate);
        yield candidate;
        enqueueVueChildren(candidate, pending);
    }
}
/** Add known Vue tree edges without invoking slots or component code. */
function enqueueVueChildren(vnode, pending) {
    const component = isRecord(vnode.component) ? vnode.component : null;
    if (component) {
        pending.push(component.subTree, component.vnode);
    }
    if (Array.isArray(vnode.children))
        pending.push(...vnode.children);
    if (Array.isArray(vnode.dynamicChildren))
        pending.push(...vnode.dynamicChildren);
    pending.push(vnode.ssContent, vnode.ssFallback);
    const suspense = isRecord(vnode.suspense) ? vnode.suspense : null;
    if (suspense)
        pending.push(suspense.activeBranch, suspense.pendingBranch);
}
