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
/** Adapt Comfy's native virtual-node and DOM-widget capabilities for the existing override card. */
import { isRecord } from '../../types/common.js';
const constructors = new WeakMap();
/** Construct a registered frontend-only node so both native renderers recognize its virtual identity. */
export function createNativeOverrideGraphNode(runtime, content, height) {
    const Constructor = overrideConstructor(runtime);
    const candidate = new Constructor();
    candidate.addDOMWidget('Global Overrides', 'sugarcubes-overrides', content, {
        hideOnZoom: false,
        serialize: false,
        margin: 0,
        getMinHeight: height,
        getMaxHeight: height,
    });
    return candidate;
}
/** Register one hidden virtual node class per native runtime through Comfy's public extension API. */
function overrideConstructor(runtime) {
    if (!isRecord(runtime) ||
        !isNativeNodeConstructor(runtime.LGraphNode) ||
        typeof runtime.registerNodeType !== 'function')
        throw new Error('ComfyUI native DOM widget nodes are unavailable for Global Overrides.');
    const existing = constructors.get(runtime);
    if (existing)
        return existing;
    const Base = runtime.LGraphNode;
    /** Supply native renderer identity while retaining all native movement and widget behavior. */
    class GlobalOverridesNode extends Base {
        static title = 'Global Overrides';
        static collapsable = false;
        static skip_list = true;
        static category = '';
        /** Keep every native instance virtual and singular, including host duplication attempts. */
        constructor() {
            super('Global Overrides');
            this.type = 'SugarCubes/GlobalOverrides';
            this.isVirtualNode = true;
            this.sugarcubesPresentationOnly = true;
            this.inputs = [];
            this.outputs = [];
            this.clonable = false;
            this.removable = false;
            this.block_delete = true;
            Object.defineProperty(this, 'collapsible', { value: false, configurable: true });
            this.collapse = preventCollapse;
        }
    }
    runtime.registerNodeType.call(runtime, 'SugarCubes/GlobalOverrides', GlobalOverridesNode);
    constructors.set(runtime, GlobalOverridesNode);
    return GlobalOverridesNode;
}
/** Validate native construction and DOM-widget methods before touching the graph. */
function isNativeNodeConstructor(value) {
    const prototype = typeof value === 'function' ? value.prototype : null;
    return (isRecord(prototype) &&
        typeof prototype.addDOMWidget === 'function' &&
        typeof prototype.setSize === 'function');
}
/** Keep the Plugin Mode card expanded while native graph movement remains available. */
function preventCollapse() { }
