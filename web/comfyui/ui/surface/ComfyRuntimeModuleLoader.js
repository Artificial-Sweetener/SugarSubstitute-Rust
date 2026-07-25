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
/** Resolve the installed Comfy runtime modules used by native Nodes 2.0. */
import { isRecord } from '../types/common.js';
const ENTRY_PATTERN = /\/assets\/index-[^/?]+\.js(?:\?.*)?$/;
const VUE_RUNTIME_PATTERN = /["']\.\/(vendor-vue-core-[^"']+\.js)["']/;
const HOST_RUNTIME_PATTERN = /["']\.\/(dialogService-[^"']+\.js)["']/;
const GRAPH_VIEW_PATTERN = /["']\.\/(GraphView-[^"']+\.js)["']/;
const NATIVE_NODE_COMPONENT_PATTERN = /([A-Za-z_$][\w$]*)=[A-Za-z_$][\w$]*\(\{__name:[`'"]LGraphNode[`'"]/;
const RELATIVE_MODULE_SPECIFIER_PATTERN = /((?:from|import)\s*(?:\(\s*)?)(["'])\.\/([^"'`]+)\2/g;
const FETCH_TIMEOUT_MS = 5_000;
const NATIVE_NODE_COMPONENT_EXPORT = 'sugarcubesNativeNodeComponent';
/** Report an installed Comfy runtime that no longer matches the guarded contract. */
export class NativeRendererCompatibilityError extends Error {
    /** Create a compatibility failure with actionable boundary context. */
    constructor(message) {
        super(message);
        this.name = 'NativeRendererCompatibilityError';
    }
}
/** Discover current hashed assets without hard-coding one Comfy build. */
export async function discoverComfyRuntimeAssets(documentRef, fetchText = fetchRuntimeText) {
    const entry = Array.from(documentRef.scripts)
        .map((script) => script.src)
        .find((source) => ENTRY_PATTERN.test(source));
    if (!entry) {
        throw new NativeRendererCompatibilityError('Comfy entry module was not found.');
    }
    const source = await fetchText(entry);
    const vueMatch = source.match(VUE_RUNTIME_PATTERN);
    const hostMatch = source.match(HOST_RUNTIME_PATTERN);
    const graphViewMatch = source.match(GRAPH_VIEW_PATTERN);
    if (!vueMatch?.[1] || !hostMatch?.[1] || !graphViewMatch?.[1]) {
        throw new NativeRendererCompatibilityError('Comfy native renderer modules do not match the supported runtime contract.');
    }
    return {
        entry,
        vueRuntime: new URL(vueMatch[1], entry).href,
        hostRuntime: new URL(hostMatch[1], entry).href,
        graphView: new URL(graphViewMatch[1], entry).href,
    };
}
/** Load Comfy's exact LGraphNode component without requiring a root-graph node. */
export async function loadComfyNativeNodeComponent(documentRef, fetchText = fetchRuntimeText, importModule = importRuntimeModule) {
    const assets = await discoverComfyRuntimeAssets(documentRef, fetchText);
    const installedSource = await fetchText(assets.graphView);
    const moduleSource = buildNativeNodeComponentModule(installedSource, assets.graphView);
    const moduleUrl = URL.createObjectURL(new Blob([moduleSource], { type: 'text/javascript' }));
    try {
        const loaded = await importModule(moduleUrl);
        const component = loaded[NATIVE_NODE_COMPONENT_EXPORT];
        if (!isRecord(component)) {
            throw new NativeRendererCompatibilityError('Comfy native node component export was not an object.');
        }
        return component;
    }
    finally {
        URL.revokeObjectURL(moduleUrl);
    }
}
/**
 * Expose the installed component while keeping every renderer dependency native.
 *
 * Comfy currently keeps LGraphNode private inside its GraphView chunk. SugarCubes
 * adds one export to that installed module and resolves its relative dependencies
 * against the original asset URL. No node markup, widgets, or styling are copied.
 */
export function buildNativeNodeComponentModule(source, graphViewUrl) {
    const componentMatch = source.match(NATIVE_NODE_COMPONENT_PATTERN);
    const componentIdentifier = componentMatch?.[1];
    if (!componentIdentifier) {
        throw new NativeRendererCompatibilityError('Comfy GraphView no longer contains the native LGraphNode component boundary.');
    }
    const resolvedSource = source.replace(RELATIVE_MODULE_SPECIFIER_PATTERN, (_match, prefix, quote, relativePath) => `${prefix}${quote}${new URL(relativePath, graphViewUrl).href}${quote}`);
    return `${resolvedSource}\nexport{${componentIdentifier} as ${NATIVE_NODE_COMPONENT_EXPORT}};\n`;
}
/** Load and capability-check the actual renderer functions from installed Comfy. */
export async function loadComfyVueRuntime(documentRef, fetchText = fetchRuntimeText, importModule = importRuntimeModule) {
    const assets = await discoverComfyRuntimeAssets(documentRef, fetchText);
    const [vueModule, hostModule] = await Promise.all([
        importModule(assets.vueRuntime),
        importModule(assets.hostRuntime),
    ]);
    const render = findNamedFunction(vueModule, 'render');
    const h = findNamedFunction(vueModule, 'h');
    const extractVueNodeData = findNamedFunction(hostModule, 'extractVueNodeData');
    const requestSlotLayoutSyncForAllNodes = findNamedFunction(hostModule, 'requestSlotLayoutSyncForAllNodes');
    if (!render || !h || !extractVueNodeData || !requestSlotLayoutSyncForAllNodes) {
        throw new NativeRendererCompatibilityError('Comfy native renderer capabilities changed; refusing a non-native fallback.');
    }
    return {
        render: (vnode, target) => {
            render(vnode, target);
        },
        h: (component, props) => h(component, props),
        extractVueNodeData: (node) => {
            const value = extractVueNodeData(node);
            if (!isRecord(value)) {
                throw new NativeRendererCompatibilityError('Comfy returned invalid native node render data.');
            }
            return value;
        },
        requestSlotLayoutSync: () => {
            requestSlotLayoutSyncForAllNodes();
        },
    };
}
/** Fetch a same-origin runtime module with an explicit timeout. */
async function fetchRuntimeText(url) {
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
    try {
        const response = await fetch(url, { signal: controller.signal });
        if (!response.ok) {
            throw new NativeRendererCompatibilityError(`Comfy runtime asset request failed with status ${response.status}.`);
        }
        return await response.text();
    }
    finally {
        window.clearTimeout(timeout);
    }
}
/** Import a same-origin installed Comfy module by its discovered URL. */
async function importRuntimeModule(url) {
    const loaded = await import(url);
    if (!isRecord(loaded)) {
        throw new NativeRendererCompatibilityError('Comfy runtime module was not an object.');
    }
    return loaded;
}
/** Resolve a minified ESM export by its retained function name. */
function findNamedFunction(module, expectedName) {
    for (const value of Object.values(module)) {
        if (typeof value === 'function' && value.name === expectedName) {
            return value;
        }
    }
    return null;
}
