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
import type { UnknownRecord } from '../types/common.js';
import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { findVueComponents } from './ComfyVueTree.js';
import { ComfyVueCubeEditorFooter } from './ComfyVueCubeEditorFooter.js';
import {
  ComfyVueCubeNodeResizeHost,
  type ComfyVueCubeNodeResizeHostOptions,
} from './ComfyVueCubeNodeResizeHost.js';
import { ComfyVueCubeBoundaryHost } from './ComfyVueCubeBoundaryHost.js';
import { resolveComfyVueCubeMinimumHeight } from './ComfyVueCubeMinimumHeight.js';
import { enforceCubeNodeMinimumSize } from './CubeNodeMinimumSizeAdapter.js';
import { cubeMinimumSize } from './CubeSurfaceMinimumHeight.js';
import type { CubePortPresentationController } from '../cube/connection/CubePortPresentationController.js';
import { ComfyVueCubeColorScope } from './ComfyVueCubeColorScope.js';

interface MountedCubeNode {
  nodeRoot: HTMLElement;
  faceHost: HTMLDivElement;
  header: HTMLElement | null;
  hiddenElements: Map<HTMLElement, boolean>;
  resizeHost: ComfyVueCubeNodeResizeHost;
  boundaryHost: ComfyVueCubeBoundaryHost;
  editorFooter: ComfyVueCubeEditorFooter;
  colorScope: ComfyVueCubeColorScope;
  observer: MutationObserver;
  geometryObserver: ResizeObserver | null;
}

interface NativeRootOwnership {
  owner: ComfyVueCubeNodeHost;
  node: CubeNode;
}

type ResizeObserverConstructor = new (callback: ResizeObserverCallback) => ResizeObserver;

const NATIVE_ROOT_OWNERS = new WeakMap<HTMLElement, NativeRootOwnership>();

export interface ComfyVueCubeNodeHostOptions
  extends Pick<ComfyVueCubeNodeResizeHostOptions, 'history' | 'getScale'> {
  document: Document;
  titleHeight: number;
  openEditor(node: CubeNode): void;
  requestSlotLayoutSync(): void;
  onGeometryChange?(node: CubeNode): void;
  portPresentation?: CubePortPresentationController;
}

/** Own only the custom-content seam inside a native Comfy node component. */
export class ComfyVueCubeNodeHost {
  readonly #document: Document;
  readonly #titleHeight: number;
  readonly #history: ComfyVueCubeNodeResizeHostOptions['history'];
  readonly #getScale: () => number;
  readonly #openEditor: (node: CubeNode) => void;
  readonly #requestSlotLayoutSync: () => void;
  readonly #onGeometryChange: (node: CubeNode) => void;
  readonly #portPresentation: CubePortPresentationController | null;
  readonly #mounts = new Map<CubeNode, MountedCubeNode>();

  /** Bind the active Comfy document and graph geometry collaborators. */
  constructor(options: ComfyVueCubeNodeHostOptions) {
    this.#document = options.document;
    this.#titleHeight = Math.max(0, options.titleHeight);
    this.#history = options.history;
    this.#getScale = options.getScale;
    this.#openEditor = options.openEditor;
    this.#requestSlotLayoutSync = options.requestSlotLayoutSync;
    this.#onGeometryChange = options.onGeometryChange ?? (() => undefined);
    this.#portPresentation = options.portPresentation ?? null;
  }

  /** Mount a face within the matching native node body, preserving native shell behavior. */
  mount(node: CubeNode): HTMLDivElement | null {
    const nodeRoot = findNativeNodeRoot(this.#document, node);
    if (!nodeRoot) return null;
    const nativeRootOwner = NATIVE_ROOT_OWNERS.get(nodeRoot);
    if (nativeRootOwner && nativeRootOwner.owner !== this) {
      nativeRootOwner.owner.unmount(nativeRootOwner.node);
    }
    const existing = this.#mounts.get(node);
    if (
      existing?.nodeRoot === nodeRoot &&
      existing.faceHost.isConnected &&
      existing.faceHost.parentElement !== null
    ) {
      removeOrphanFaceHosts(existing.faceHost.parentElement, existing.faceHost);
      existing.colorScope.refresh();
      return existing.faceHost;
    }
    if (existing) this.unmount(node);
    for (const [mountedNode, mount] of this.#mounts) {
      if (mount.nodeRoot === nodeRoot) this.unmount(mountedNode);
    }

    const body = findNativeNodeBody(nodeRoot);
    if (!body) return null;
    body.dataset.sugarcubeCubeBody = '';
    const colorScope = new ComfyVueCubeColorScope(nodeRoot);
    colorScope.mount();
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
      onGeometryChange: () => this.#onGeometryChange(node),
    });
    const boundaryHost = new ComfyVueCubeBoundaryHost({
      body,
      node,
      titleHeight: this.#titleHeight,
      requestSlotLayoutSync: this.#requestSlotLayoutSync,
      ...(this.#portPresentation ? { portPresentation: this.#portPresentation } : {}),
    });
    const editorFooter = new ComfyVueCubeEditorFooter(nodeRoot, node, this.#openEditor);
    const observer = new MutationObserver((records) => {
      const mounted = this.#mounts.get(node);
      if (mounted && records.some((record) => requiresNativeHostReconcile(record, mounted))) {
        this.#reconcile(node, mounted);
      }
    });
    const viewRecord = isRecord(this.#document.defaultView) ? this.#document.defaultView : null;
    const ResizeObserverValue: unknown = viewRecord?.ResizeObserver;
    const geometryObserver =
      typeof ResizeObserverValue === 'function'
        ? // Browser globals are validated here because detached test windows omit this constructor.
          new (ResizeObserverValue as ResizeObserverConstructor)(() => this.#onGeometryChange(node))
        : null;
    const mounted = {
      nodeRoot,
      faceHost,
      header: null,
      hiddenElements,
      resizeHost,
      boundaryHost,
      editorFooter,
      colorScope,
      observer,
      geometryObserver,
    };
    this.#mounts.set(node, mounted);
    NATIVE_ROOT_OWNERS.set(nodeRoot, { owner: this, node });
    observer.observe(nodeRoot, { childList: true, subtree: true });
    geometryObserver?.observe(nodeRoot);
    return faceHost;
  }

  /** Mount Cube chrome inside Comfy's actual header interaction surface. */
  mountHeader(node: CubeNode, header: HTMLElement): boolean {
    const mount = this.#mounts.get(node);
    if (!mount) return false;
    if (mount.header && mount.header !== header) mount.header.remove();
    mount.header = header;
    return reconcileNativeHeader(mount);
  }

  /** Return the face host mounted within one native node. */
  getRoot(node: CubeNode): HTMLDivElement | null {
    return this.#mounts.get(node)?.faceHost ?? null;
  }

  /** Remeasure boundary anchors after Cube-owned face content changes. */
  reconcileBoundary(node: CubeNode): void {
    this.#mounts.get(node)?.boundaryHost.reconcile();
  }

  /** Synchronize measured face constraints with native and supplemental resizing. */
  reconcileMinimumSize(node: CubeNode, minimumHeight: number): boolean {
    const mount = this.#mounts.get(node);
    if (!mount) return false;
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
    const minimumSize = cubeMinimumSize(resolvedMinimumHeight);
    return enforceCubeNodeMinimumSize(node, [Math.max(1, Number(node.size[0])), minimumSize[1]]);
  }

  /** Restore the generic native body when the custom face is released. */
  unmount(node: CubeNode): void {
    const mount = this.#mounts.get(node);
    if (!mount) return;
    mount.observer.disconnect();
    mount.geometryObserver?.disconnect();
    for (const [element, wasHidden] of mount.hiddenElements) {
      if (element.isConnected) element.hidden = wasHidden;
      element.removeAttribute('data-sugarcube-native-hidden');
    }
    mount.resizeHost.dispose();
    mount.boundaryHost.dispose();
    mount.editorFooter.dispose();
    mount.colorScope.dispose();
    mount.header?.remove();
    mount.faceHost.remove();
    const body = findNativeNodeBody(mount.nodeRoot);
    if (body) delete body.dataset.sugarcubeCubeBody;
    delete mount.nodeRoot.dataset.sugarcubeNode;
    this.#mounts.delete(node);
    const nativeRootOwner = NATIVE_ROOT_OWNERS.get(mount.nodeRoot);
    if (nativeRootOwner?.owner === this && nativeRootOwner.node === node) {
      NATIVE_ROOT_OWNERS.delete(mount.nodeRoot);
    }
  }

  /** Release every native-node face seam. */
  dispose(): void {
    for (const node of [...this.#mounts.keys()]) this.unmount(node);
  }

  /** Repair only host seams that native Vue reconciliation can replace. */
  #reconcile(node: CubeNode, mount: MountedCubeNode): void {
    if (mount.nodeRoot !== findNativeNodeRoot(this.#document, node)) return;
    const body = findNativeNodeBody(mount.nodeRoot);
    if (!body) return;
    body.dataset.sugarcubeCubeBody = '';
    if (mount.faceHost.parentElement !== body) body.append(mount.faceHost);
    for (const [element, wasHidden] of hideNativeCubeContent(mount.nodeRoot, body)) {
      if (!mount.hiddenElements.has(element)) mount.hiddenElements.set(element, wasHidden);
    }
    mount.resizeHost.ensureMounted();
    mount.boundaryHost.reconcile();
    mount.editorFooter.reconcile();
    mount.colorScope.refresh();
    reconcileNativeHeader(mount);
  }
}

/** Remove stale extension-owned hosts while preserving one current owner when supplied. */
function removeOrphanFaceHosts(body: HTMLElement, preserve?: HTMLElement): void {
  for (const child of [...body.children]) {
    if (
      child instanceof HTMLElement &&
      child.dataset.sugarcubeFaceHost !== undefined &&
      child !== preserve
    ) {
      child.remove();
    }
  }
}

/** Ignore Cube-owned face mutations while retaining repairs for native shell replacement. */
function requiresNativeHostReconcile(record: MutationRecord, mount: MountedCubeNode): boolean {
  const target = record.target;
  if (target === mount.faceHost || mount.faceHost.contains(target)) return false;
  if (mount.header && (target === mount.header || mount.header.contains(target))) return false;
  if (!(target instanceof Element)) return true;
  return (
    target.closest(
      '[data-sugarcube-edge-resize], [data-sugarcube-port-leaders], [data-sugarcube-output-leader]',
    ) === null
  );
}

/** Locate the exact Nodes 2.0 root by native graph-node identity. */
function findNativeNodeRoot(documentRef: Document, node: CubeNode): HTMLElement | null {
  const identity = String(node.id);
  for (const element of documentRef.querySelectorAll<HTMLElement>('.lg-node[data-node-id]')) {
    if (element.dataset.nodeId === identity) return element;
  }
  return null;
}

/** Resolve the native expanded body that owns slots and custom content. */
function findNativeNodeBody(nodeRoot: HTMLElement): HTMLElement | null {
  return nodeRoot.querySelector<HTMLElement>('[data-testid^="node-body-"]');
}

/** Hide generic subgraph content while retaining native slots, borders, and resize handles. */
function hideNativeCubeContent(
  nodeRoot: HTMLElement,
  body: HTMLElement,
): Map<HTMLElement, boolean> {
  const hidden = new Map<HTMLElement, boolean>();
  const nativeHeader = findNativeNodeHeader(nodeRoot);
  const rootRecord: UnknownRecord = isRecord(nodeRoot) ? nodeRoot : {};
  const componentTree = rootRecord.__vueParentComponent;
  for (const componentName of ['NodeWidgets', 'NodeContent', 'LivePreview', 'NodeBadges']) {
    for (const vnode of findVueComponents(componentTree, componentName)) {
      const element = resolveComponentElement(vnode);
      if (element && !ownsNativeHeader(element, nativeHeader)) hideElement(element, hidden);
    }
  }

  for (const child of [...body.children]) {
    if (!(child instanceof HTMLElement) || child.dataset.sugarcubeFaceHost !== undefined) continue;
    if (containsNativeSlot(child)) continue;
    hideElement(child, hidden);
  }
  return hidden;
}

/** Replace only native header contents while retaining Comfy's drag owner. */
function reconcileNativeHeader(mount: MountedCubeNode): boolean {
  if (!mount.header) return false;
  const nativeHeader = findNativeNodeHeader(mount.nodeRoot);
  if (!nativeHeader) return false;
  restoreElement(nativeHeader, mount.hiddenElements);
  for (const child of [...nativeHeader.children]) {
    if (child instanceof HTMLElement && child !== mount.header) {
      hideElement(child, mount.hiddenElements);
    }
  }
  if (mount.header.parentElement !== nativeHeader) nativeHeader.append(mount.header);
  return true;
}

/** Locate the owning root node's header without selecting projected child-card headers. */
function findNativeNodeHeader(nodeRoot: HTMLElement): HTMLElement | null {
  const expectedTestId = `node-header-${nodeRoot.dataset.nodeId ?? ''}`;
  for (const candidate of nodeRoot.querySelectorAll<HTMLElement>('[data-testid^="node-header-"]')) {
    if (candidate.getAttribute('data-testid') === expectedTestId) return candidate;
  }
  return null;
}

/** Measure one explicit native flow owner without deriving it from face position. */
function readFlowHeight(element: HTMLElement | null): number {
  const view = element?.ownerDocument.defaultView;
  if (!element || !view) return 0;
  const styles = view.getComputedStyle(element);
  return Math.max(
    0,
    finiteLength(styles.marginTop) + element.offsetHeight + finiteLength(styles.marginBottom),
  );
}

/** Parse a computed length without allowing invalid host geometry to propagate. */
function finiteLength(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

/** Preserve the native header and any host wrapper that owns it. */
function ownsNativeHeader(element: HTMLElement, nativeHeader: HTMLElement | null): boolean {
  return nativeHeader !== null && (element === nativeHeader || element.contains(nativeHeader));
}

/** Retain the Nodes 2.0 slot components used for native noodle interaction. */
function containsNativeSlot(element: HTMLElement): boolean {
  return (
    element.matches(
      '.lg-slot, [data-testid="slot-connection-dot"], [data-node-id][data-slot-index], [data-slot-index]',
    ) ||
    element.querySelector(
      '.lg-slot, [data-testid="slot-connection-dot"], [data-node-id][data-slot-index], [data-slot-index]',
    ) !== null
  );
}

/** Hide one host-owned element while retaining its prior state for restoration. */
function hideElement(element: HTMLElement, hidden: Map<HTMLElement, boolean>): void {
  if (hidden.has(element)) return;
  hidden.set(element, element.hidden);
  element.hidden = true;
  element.dataset.sugarcubeNativeHidden = '';
}

/** Restore an element that belongs to the retained native shell. */
function restoreElement(element: HTMLElement, hidden: Map<HTMLElement, boolean>): void {
  const wasHidden = hidden.get(element);
  if (wasHidden === undefined) return;
  element.hidden = wasHidden;
  element.removeAttribute('data-sugarcube-native-hidden');
  hidden.delete(element);
}

/** Resolve the rendered root element for one mounted Vue component VNode. */
function resolveComponentElement(vnode: UnknownRecord): HTMLElement | null {
  const component = isRecord(vnode.component) ? vnode.component : null;
  const subTree = isRecord(component?.subTree) ? component.subTree : null;
  return subTree?.el instanceof HTMLElement ? subTree.el : null;
}
