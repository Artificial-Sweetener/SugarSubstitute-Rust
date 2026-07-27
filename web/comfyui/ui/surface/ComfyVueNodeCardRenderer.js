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
/** Mount Comfy's real Nodes 2.0 component as a Cube-face sub-card. */
import { isRecord } from '../types/common.js';
import { findVueComponent } from './ComfyVueTree.js';
import { createCubeFaceNodeData, cubeFaceNodeHasVisibleWidgets, } from './CubeFaceNodePresentationPolicy.js';
import { findCubeFacePromptWidget } from './CubeFacePromptPolicy.js';
import { fitCubeFacePromptTextarea } from './CubeFacePromptTextarea.js';
/** Own native Vue mounts while leaving component rendering and widget UI to Comfy. */
export class ComfyVueNodeCardRenderer {
    #component;
    #appContext;
    #runtime;
    #mounts = new Set();
    #interactiveRoots = new WeakSet();
    #promptTextareas = new WeakSet();
    /** Bind the installed Comfy component, application context, and renderer functions. */
    constructor(options) {
        this.#component = options.component;
        this.#appContext = options.appContext;
        this.#runtime = options.runtime;
    }
    /** Mount one exact internal graph node through Comfy's active native renderer. */
    mount(target, node) {
        let disposed = false;
        const observer = new MutationObserver(() => {
            if (!disposed)
                this.#applyPresentation(target, node);
        });
        const renderCard = () => {
            if (disposed)
                return;
            const nodeData = createCubeFaceNodeData(this.#runtime.extractVueNodeData(node));
            const vnodeValue = this.#runtime.h(this.#component, { nodeData });
            if (!isRecord(vnodeValue)) {
                throw new TypeError('Comfy native renderer returned an invalid VNode.');
            }
            vnodeValue.appContext = this.#appContext;
            this.#runtime.render(vnodeValue, target);
            target.classList.add('sugarcubes-native-node-card');
            target.dataset.cubeFaceNative = 'nodes-2';
            this.#applyPresentation(target, node);
        };
        renderCard();
        observer.observe(target, { childList: true, subtree: true });
        const mount = {
            refresh: renderCard,
            unmount: () => {
                if (disposed)
                    return;
                disposed = true;
                observer.disconnect();
                this.#runtime.render(null, target);
                this.#mounts.delete(mount);
            },
        };
        this.#mounts.add(mount);
        return mount;
    }
    /** Unmount every card created by this renderer instance. */
    dispose() {
        for (const mount of [...this.#mounts])
            mount.unmount();
    }
    /** Apply the narrow Cube-face mode to Comfy-owned component roots. */
    #applyPresentation(target, node) {
        const nativeRoot = target.querySelector('.lg-node');
        if (nativeRoot) {
            nativeRoot.dataset.cubeFacePresentation = 'true';
            for (const handle of nativeRoot.querySelectorAll(':scope > [role="button"]')) {
                handle.hidden = true;
            }
            const collapseButton = nativeRoot.querySelector('[data-testid="node-collapse-button"]');
            if (collapseButton)
                hideComponentRoot(collapseButton);
            if (node.isSubgraphNode?.()) {
                hideNativeSubgraphIcon(nativeRoot);
                hideNativeSubgraphFooter(nativeRoot);
            }
        }
        const targetRecord = isRecord(target) ? target : {};
        for (const componentName of [
            'NodeSlots',
            'NodeContent',
            'LivePreview',
            'ImagePreview',
            'NodeBadges',
            'NodeFooter',
        ]) {
            const vnode = findVueComponent(targetRecord._vnode, componentName);
            const element = resolveComponentElement(vnode);
            if (element)
                hideComponentRoot(element);
        }
        if (nativeRoot)
            reconcileNativeBody(nativeRoot, node);
        this.#fitPromptTextareas(target, node);
        const widgetsVNode = findVueComponent(targetRecord._vnode, 'NodeWidgets');
        const widgetsRoot = resolveComponentElement(widgetsVNode);
        if (widgetsRoot && !this.#interactiveRoots.has(widgetsRoot)) {
            const stopAtWidgets = (event) => event.stopPropagation();
            widgetsRoot.addEventListener('pointerdown', stopAtWidgets);
            widgetsRoot.addEventListener('mousedown', stopAtWidgets);
            widgetsRoot.addEventListener('wheel', stopAtWidgets);
            widgetsRoot.addEventListener('contextmenu', stopAtWidgets);
            this.#interactiveRoots.add(widgetsRoot);
        }
    }
    /** Let only semantic prompt editors grow their card instead of scrolling in place. */
    #fitPromptTextareas(target, node) {
        if (!findCubeFacePromptWidget(node))
            return;
        for (const textarea of target.querySelectorAll('textarea')) {
            const fit = () => {
                fitCubeFacePromptTextarea(textarea);
            };
            if (!this.#promptTextareas.has(textarea)) {
                textarea.addEventListener('input', fit);
                this.#promptTextareas.add(textarea);
            }
            fit();
        }
    }
}
/** Remove the native body when Cube-face policy leaves only the title row. */
function reconcileNativeBody(nativeRoot, node) {
    const body = nativeRoot.querySelector('[data-testid^="node-body-"]');
    if (!body)
        return;
    const headerOnly = !cubeFaceNodeHasVisibleWidgets(node);
    const wasHeaderOnly = nativeRoot.dataset.cubeFaceBody === 'header-only';
    if (headerOnly) {
        nativeRoot.dataset.cubeFaceBody = 'header-only';
        hideComponentRoot(body);
        return;
    }
    if (!wasHeaderOnly)
        return;
    delete nativeRoot.dataset.cubeFaceBody;
    body.hidden = false;
    body.style.removeProperty('display');
}
/** Hide Comfy's subgraph action footer while retaining the complete native card above it. */
function hideNativeSubgraphFooter(nativeRoot) {
    const testIdButton = nativeRoot.querySelector('[data-testid="subgraph-enter-button"]');
    const footer = testIdButton?.parentElement ??
        nativeRoot.querySelector(':scope > .isolate.-z-1.-mt-5');
    if (footer)
        hideComponentRoot(footer);
}
/** Hide the native subgraph glyph from one projected Cube-face card header. */
function hideNativeSubgraphIcon(nativeRoot) {
    for (const element of nativeRoot.querySelectorAll('*')) {
        if (element.classList.contains('icon-[comfy--workflow]'))
            hideComponentRoot(element);
    }
}
/** Hide one Comfy-owned presentation root without replacing its renderer. */
function hideComponentRoot(element) {
    element.hidden = true;
    element.style.setProperty('display', 'none', 'important');
}
/** Resolve the rendered root element for one mounted component VNode. */
function resolveComponentElement(vnode) {
    const component = isRecord(vnode?.component) ? vnode.component : null;
    const subTree = isRecord(component?.subTree) ? component.subTree : null;
    return subTree?.el instanceof HTMLElement ? subTree.el : null;
}
