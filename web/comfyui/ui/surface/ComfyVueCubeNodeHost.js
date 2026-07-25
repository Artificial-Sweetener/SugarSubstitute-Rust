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
/** Mount a Cube face inside Comfy's real Nodes 2.0 node root. */
import { isRecord } from '../types/common.js';
import { findVueComponents } from './ComfyVueTree.js';
import { ComfyVueCubeEditorFooter } from './ComfyVueCubeEditorFooter.js';
import { ComfyVueCubeNodeResizeHost, } from './ComfyVueCubeNodeResizeHost.js';
import { ComfyVueCubeBoundaryHost } from './ComfyVueCubeBoundaryHost.js';
import { resolveComfyVueCubeMinimumHeight } from './ComfyVueCubeMinimumHeight.js';
import { enforceCubeNodeMinimumHeight } from './CubeNodeMinimumHeightAdapter.js';
const NATIVE_ROOT_OWNERS = new WeakMap();
/** Own only the custom-content seam inside a native Comfy node component. */
export class ComfyVueCubeNodeHost {
    #document;
    #history;
    #getScale;
    #openEditor;
    #requestSlotLayoutSync;
    #mounts = new Map();
    /** Bind the active Comfy document and graph geometry collaborators. */
    constructor(options) {
        this.#document = options.document;
        this.#history = options.history;
        this.#getScale = options.getScale;
        this.#openEditor = options.openEditor;
        this.#requestSlotLayoutSync = options.requestSlotLayoutSync;
    }
    /** Mount a face within the matching native node body, preserving native shell behavior. */
    mount(node) {
        const nodeRoot = findNativeNodeRoot(this.#document, node);
        if (!nodeRoot)
            return null;
        const nativeRootOwner = NATIVE_ROOT_OWNERS.get(nodeRoot);
        if (nativeRootOwner && nativeRootOwner.owner !== this) {
            nativeRootOwner.owner.unmount(nativeRootOwner.node);
        }
        const existing = this.#mounts.get(node);
        if (existing?.nodeRoot === nodeRoot &&
            existing.faceHost.isConnected &&
            existing.faceHost.parentElement !== null) {
            removeOrphanFaceHosts(existing.faceHost.parentElement, existing.faceHost);
            this.#reconcile(node, existing);
            return existing.faceHost;
        }
        if (existing)
            this.unmount(node);
        for (const [mountedNode, mount] of this.#mounts) {
            if (mount.nodeRoot === nodeRoot)
                this.unmount(mountedNode);
        }
        const body = findNativeNodeBody(nodeRoot);
        if (!body)
            return null;
        body.dataset.sugarcubeCubeBody = '';
        removeOrphanFaceHosts(body);
        const faceHost = this.#document.createElement('div');
        faceHost.className = 'sugarcubes-cube-node-face-host';
        faceHost.dataset.sugarcubeFaceHost = '';
        body.append(faceHost);
        nodeRoot.dataset.sugarcubeNode = 'true';
        const hiddenElements = hideNativeCubeContent(nodeRoot, body);
        const resizeHost = new ComfyVueCubeNodeResizeHost({
            root: nodeRoot,
            node,
            history: this.#history,
            getScale: this.#getScale,
        });
        const boundaryHost = new ComfyVueCubeBoundaryHost(body, this.#requestSlotLayoutSync);
        const editorFooter = new ComfyVueCubeEditorFooter(nodeRoot, node, this.#openEditor);
        const observer = new MutationObserver(() => {
            const mounted = this.#mounts.get(node);
            if (mounted)
                this.#reconcile(node, mounted);
        });
        const mounted = {
            nodeRoot,
            faceHost,
            header: null,
            hiddenElements,
            resizeHost,
            boundaryHost,
            editorFooter,
            observer,
        };
        this.#mounts.set(node, mounted);
        NATIVE_ROOT_OWNERS.set(nodeRoot, { owner: this, node });
        observer.observe(nodeRoot, { childList: true, subtree: true });
        return faceHost;
    }
    /** Mount Cube chrome inside Comfy's actual header interaction surface. */
    mountHeader(node, header) {
        const mount = this.#mounts.get(node);
        if (!mount)
            return false;
        if (mount.header && mount.header !== header)
            mount.header.remove();
        mount.header = header;
        return reconcileNativeHeader(mount);
    }
    /** Return the face host mounted within one native node. */
    getRoot(node) {
        return this.#mounts.get(node)?.faceHost ?? null;
    }
    /** Synchronize measured face constraints with native and supplemental resizing. */
    reconcileMinimumHeight(node, minimumHeight) {
        const mount = this.#mounts.get(node);
        if (!mount)
            return false;
        const body = findNativeNodeBody(mount.nodeRoot);
        const resolvedMinimumHeight = body
            ? resolveComfyVueCubeMinimumHeight({
                faceMinimumHeight: minimumHeight,
                headerFlowHeight: readFlowHeight(findNativeNodeHeader(mount.nodeRoot)),
                footerFlowHeight: mount.editorFooter.getFlowHeight(),
                nodeHeight: Number(node.size[1]),
                nodeRoot: mount.nodeRoot,
            })
            : minimumHeight;
        mount.resizeHost.setMinimumHeight(resolvedMinimumHeight);
        return enforceCubeNodeMinimumHeight(node, resolvedMinimumHeight);
    }
    /** Restore the generic native body when the custom face is released. */
    unmount(node) {
        const mount = this.#mounts.get(node);
        if (!mount)
            return;
        mount.observer.disconnect();
        for (const [element, wasHidden] of mount.hiddenElements) {
            if (element.isConnected)
                element.hidden = wasHidden;
            element.removeAttribute('data-sugarcube-native-hidden');
        }
        mount.resizeHost.dispose();
        mount.boundaryHost.dispose();
        mount.editorFooter.dispose();
        mount.header?.remove();
        mount.faceHost.remove();
        const body = findNativeNodeBody(mount.nodeRoot);
        if (body)
            delete body.dataset.sugarcubeCubeBody;
        delete mount.nodeRoot.dataset.sugarcubeNode;
        this.#mounts.delete(node);
        const nativeRootOwner = NATIVE_ROOT_OWNERS.get(mount.nodeRoot);
        if (nativeRootOwner?.owner === this && nativeRootOwner.node === node) {
            NATIVE_ROOT_OWNERS.delete(mount.nodeRoot);
        }
    }
    /** Release every native-node face seam. */
    dispose() {
        for (const node of [...this.#mounts.keys()])
            this.unmount(node);
    }
    /** Repair only host seams that native Vue reconciliation can replace. */
    #reconcile(node, mount) {
        if (mount.nodeRoot !== findNativeNodeRoot(this.#document, node))
            return;
        const body = findNativeNodeBody(mount.nodeRoot);
        if (!body)
            return;
        body.dataset.sugarcubeCubeBody = '';
        if (mount.faceHost.parentElement !== body)
            body.append(mount.faceHost);
        for (const [element, wasHidden] of hideNativeCubeContent(mount.nodeRoot, body)) {
            if (!mount.hiddenElements.has(element))
                mount.hiddenElements.set(element, wasHidden);
        }
        mount.resizeHost.ensureMounted();
        mount.boundaryHost.reconcile();
        mount.editorFooter.reconcile();
        reconcileNativeHeader(mount);
    }
}
/** Remove stale extension-owned hosts while preserving one current owner when supplied. */
function removeOrphanFaceHosts(body, preserve) {
    for (const child of [...body.children]) {
        if (child instanceof HTMLElement &&
            child.dataset.sugarcubeFaceHost !== undefined &&
            child !== preserve) {
            child.remove();
        }
    }
}
/** Locate the exact Nodes 2.0 root by native graph-node identity. */
function findNativeNodeRoot(documentRef, node) {
    const identity = String(node.id);
    for (const element of documentRef.querySelectorAll('.lg-node[data-node-id]')) {
        if (element.dataset.nodeId === identity)
            return element;
    }
    return null;
}
/** Resolve the native expanded body that owns slots and custom content. */
function findNativeNodeBody(nodeRoot) {
    return nodeRoot.querySelector('[data-testid^="node-body-"]');
}
/** Hide generic subgraph content while retaining native slots, borders, and resize handles. */
function hideNativeCubeContent(nodeRoot, body) {
    const hidden = new Map();
    const nativeHeader = findNativeNodeHeader(nodeRoot);
    const rootRecord = isRecord(nodeRoot) ? nodeRoot : {};
    const componentTree = rootRecord.__vueParentComponent;
    for (const componentName of ['NodeWidgets', 'NodeContent', 'LivePreview', 'NodeBadges']) {
        for (const vnode of findVueComponents(componentTree, componentName)) {
            const element = resolveComponentElement(vnode);
            if (element && !ownsNativeHeader(element, nativeHeader))
                hideElement(element, hidden);
        }
    }
    for (const child of [...body.children]) {
        if (!(child instanceof HTMLElement) || child.dataset.sugarcubeFaceHost !== undefined)
            continue;
        if (containsNativeSlot(child))
            continue;
        hideElement(child, hidden);
    }
    return hidden;
}
/** Replace only native header contents while retaining Comfy's drag owner. */
function reconcileNativeHeader(mount) {
    if (!mount.header)
        return false;
    const nativeHeader = findNativeNodeHeader(mount.nodeRoot);
    if (!nativeHeader)
        return false;
    restoreElement(nativeHeader, mount.hiddenElements);
    for (const child of [...nativeHeader.children]) {
        if (child instanceof HTMLElement && child !== mount.header) {
            hideElement(child, mount.hiddenElements);
        }
    }
    if (mount.header.parentElement !== nativeHeader)
        nativeHeader.append(mount.header);
    return true;
}
/** Locate the owning root node's header without selecting projected child-card headers. */
function findNativeNodeHeader(nodeRoot) {
    const expectedTestId = `node-header-${nodeRoot.dataset.nodeId ?? ''}`;
    for (const candidate of nodeRoot.querySelectorAll('[data-testid^="node-header-"]')) {
        if (candidate.getAttribute('data-testid') === expectedTestId)
            return candidate;
    }
    return null;
}
/** Measure one explicit native flow owner without deriving it from face position. */
function readFlowHeight(element) {
    const view = element?.ownerDocument.defaultView;
    if (!element || !view)
        return 0;
    const styles = view.getComputedStyle(element);
    return Math.max(0, finiteLength(styles.marginTop) + element.offsetHeight + finiteLength(styles.marginBottom));
}
/** Parse a computed length without allowing invalid host geometry to propagate. */
function finiteLength(value) {
    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) ? parsed : 0;
}
/** Preserve the native header and any host wrapper that owns it. */
function ownsNativeHeader(element, nativeHeader) {
    return nativeHeader !== null && (element === nativeHeader || element.contains(nativeHeader));
}
/** Retain the Nodes 2.0 slot components used for native noodle interaction. */
function containsNativeSlot(element) {
    return (element.matches('.lg-slot, [data-testid="slot-connection-dot"], [data-node-id][data-slot-index], [data-slot-index]') ||
        element.querySelector('.lg-slot, [data-testid="slot-connection-dot"], [data-node-id][data-slot-index], [data-slot-index]') !== null);
}
/** Hide one host-owned element while retaining its prior state for restoration. */
function hideElement(element, hidden) {
    if (hidden.has(element))
        return;
    hidden.set(element, element.hidden);
    element.hidden = true;
    element.dataset.sugarcubeNativeHidden = '';
}
/** Restore an element that belongs to the retained native shell. */
function restoreElement(element, hidden) {
    const wasHidden = hidden.get(element);
    if (wasHidden === undefined)
        return;
    element.hidden = wasHidden;
    element.removeAttribute('data-sugarcube-native-hidden');
    hidden.delete(element);
}
/** Resolve the rendered root element for one mounted Vue component VNode. */
function resolveComponentElement(vnode) {
    const component = isRecord(vnode.component) ? vnode.component : null;
    const subTree = isRecord(component?.subTree) ? component.subTree : null;
    return subTree?.el instanceof HTMLElement ? subTree.el : null;
}
