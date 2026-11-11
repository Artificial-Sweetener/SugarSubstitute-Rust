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
/** Mount one existing override card inside a movable, unconnected native graph node. */
import { isRecord } from '../../types/common.js';
import { identifyMasonrySupplement, releaseMasonrySupplement, } from '../../surface/masonry/MasonrySupplementBoundary.js';
import { relocateControl } from '../../core/ControlPlacement.js';
import { createNativeOverrideGraphNode, isNativeOverrideGraphNode, } from './NativeOverrideGraphNode.js';
import { captureOverrideGraphLayout, readOverrideGraphLayout, writeOverrideGraphLayout, } from './OverrideGraphLayout.js';
import { placeNativeOverridePin } from './NativeOverridePinPlacement.js';
/** Delegate pan, zoom, movement and DOM-widget lifetime to Comfy's graph renderer. */
export class NativeOverrideGraphSurface {
    document;
    application;
    runtime;
    #graph = null;
    #node = null;
    #content = null;
    #root = null;
    #observer = null;
    #height = 100;
    #borrowed = false;
    #previousSerialize = null;
    /** Inject host access explicitly so graph placement never discovers application globals. */
    constructor(document, application, runtime) {
        this.document = document;
        this.application = application;
        this.runtime = runtime;
    }
    /** Reuse the mounted card and create only the native graph container when needed. */
    attach(root) {
        const app = this.application();
        const graph = app?.graph;
        if (!graph?.add || !graph.remove) {
            this.detach();
            return false;
        }
        if (graph === this.#graph && this.#node && graph._nodes?.includes(this.#node)) {
            if (this.#content && root.parentElement !== this.#content)
                relocateControl(root, this.#content);
            this.#borrowed = false;
            root.dataset.scOverridesGraph = '';
            this.#observer?.observe(root);
            this.#resize();
            return true;
        }
        this.detach();
        const content = this.document.createElement('div');
        content.className = 'sc-overrides-graph-content';
        const node = createNativeOverrideGraphNode(this.runtime(), content, () => this.#height);
        const saved = readOverrideGraphLayout(graph.extra);
        node.pos = saved ? [saved.x, saved.y] : graphOrigin(app?.canvas?.ds);
        node.setSize([saved?.width ?? 400, this.#height + 40]);
        this.#graph = graph;
        this.#node = node;
        this.#content = content;
        this.#root = root;
        identifyMasonrySupplement(root, content, 'plugin:global-overrides');
        this.#previousSerialize = graph.onSerialize;
        graph.onSerialize = this.#serialize;
        this.#removeStaleNodes(graph, node);
        relocateControl(root, content);
        root.dataset.scOverridesGraph = '';
        root.style.maxWidth = '100%';
        graph.add(node);
        const Observer = this.document.defaultView?.ResizeObserver;
        if (Observer) {
            this.#observer = new Observer(() => this.#resize());
            this.#observer.observe(root);
        }
        this.#resize();
        return true;
    }
    /** Lend the existing controls while retaining the native frame as the shared animation endpoint. */
    showInMasonry(root, slot) {
        const graph = this.application()?.graph;
        if (graph !== this.#graph || !this.#node || !graph?._nodes?.includes(this.#node))
            if (!this.attach(root))
                return false;
        this.#borrowed = true;
        root.removeAttribute('data-sc-overrides-graph');
        if (root.parentElement !== slot)
            relocateControl(root, slot);
        this.#resize();
        return true;
    }
    /** Preserve native placement and release the container after moving back to masonry or chrome. */
    detach() {
        this.#observer?.disconnect();
        this.#observer = null;
        this.#borrowed = false;
        if (this.#root && this.#content)
            releaseMasonrySupplement(this.#root, this.#content);
        const graph = this.#graph, node = this.#node;
        if (graph && node) {
            graph.extra = writeOverrideGraphLayout(graph.extra, captureOverrideGraphLayout(node));
            this.#root?.removeAttribute('data-sc-overrides-graph');
            if (this.#root?.parentElement === this.#content)
                this.#root.remove();
            if (graph._nodes?.includes(node))
                graph.remove?.(node);
            this.#removeStaleNodes(graph, null);
            if (graph.onSerialize === this.#serialize)
                graph.onSerialize = this.#previousSerialize;
        }
        const current = this.application()?.graph;
        if (current && current !== graph)
            this.#removeStaleNodes(current, null);
        this.#content?.remove();
        this.#graph = null;
        this.#node = null;
        this.#root = null;
        this.#content = null;
        this.#previousSerialize = null;
    }
    /** Release the old workflow before Comfy reuses its graph object for another document. */
    beforeConfigureGraph() {
        this.detach();
    }
    /** Reconcile restored or pasted cards only when the workflow topology changes. */
    reconcile() {
        const graph = this.application()?.graph;
        if (graph)
            this.#removeStaleNodes(graph, graph === this.#graph ? this.#node : null);
    }
    /** Retain earlier serialization hooks and keep this presentation node out of saved executable data. */
    #serialize = (data) => {
        if (typeof this.#previousSerialize === 'function')
            this.#previousSerialize.call(this.#graph, data);
        if (!isRecord(data))
            return;
        if (Array.isArray(data.nodes))
            data.nodes = data.nodes.filter((node) => !isNativeOverrideGraphNode(node));
        if (this.#node)
            data.extra = writeOverrideGraphLayout(data.extra, captureOverrideGraphLayout(this.#node));
    };
    /** Repair restored or pasted presentation copies while preserving the authoritative live card. */
    #removeStaleNodes(graph, live) {
        for (const node of [...(graph._nodes ?? [])])
            if (node !== live && isNativeOverrideGraphNode(node))
                graph.remove?.(node);
    }
    /** Size the native DOM widget from unscaled content dimensions without following graph zoom. */
    #resize() {
        const root = this.#root, node = this.#node;
        if (!root || !node || root.closest('[data-sc-geometry-moving]'))
            return;
        placeNativeOverridePin(root, node, this.application(), this.runtime());
        const content = this.#borrowed ? root.querySelector('.sc-overrides-fields') : root;
        const height = Math.max(40, content?.scrollHeight ?? 0);
        if (height === this.#height)
            return;
        this.#height = height;
        node.setSize([node.size?.[0] ?? 400, height + 40]);
        this.#graph?.setDirtyCanvas?.(true, true);
    }
}
/** Place the initial card inside the native viewport using its pan and zoom transform. */
function graphOrigin(transform) {
    const ds = isRecord(transform) ? transform : {};
    const scale = typeof ds.scale === 'number' && ds.scale > 0 ? ds.scale : 1;
    const offset = isRecord(ds.offset) ? ds.offset : {};
    const x = typeof offset[0] === 'number' ? offset[0] : 0;
    const y = typeof offset[1] === 'number' ? offset[1] : 0;
    return [32 / scale - x, 96 / scale - y];
}
