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
import type { UnknownRecord } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';
import type { ComfyVueNodeRenderRuntime } from './ComfyRuntimeModuleLoader.js';
import { findVueComponent, findVueComponents } from './ComfyVueTree.js';
import {
  createCubeFaceNodeData,
  cubeFaceNodeHasVisibleWidgets,
} from './CubeFaceNodePresentationPolicy.js';
import { findCubeFacePromptWidget } from './CubeFacePromptPolicy.js';
import { CubePromptTextareaScope } from './CubePromptTextareaScope.js';
import { ComfyVueNodeHeaderAccessoryHost } from './ComfyVueNodeHeaderAccessoryHost.js';
import { NativeProjectedNodeCollapse } from './NativeProjectedNodeCollapse.js';
import { NativeProjectedNodeScrollBoundary } from './NativeProjectedNodeScrollBoundary.js';
import { NATIVE_NODE_CARD_DISPOSE_EVENT } from './NativeNodeCardRenderer.js';
import type {
  NativeNodeCardMount,
  NativeNodeCardMountOptions,
  NativeNodeCardRenderer,
} from './NativeNodeCardRenderer.js';

export interface ComfyVueNodeCardRendererOptions {
  component: UnknownRecord;
  appContext: UnknownRecord;
  runtime: ComfyVueNodeRenderRuntime;
}

/** Own native Vue mounts while leaving component rendering and widget UI to Comfy. */
export class ComfyVueNodeCardRenderer implements NativeNodeCardRenderer {
  readonly #component: UnknownRecord;
  readonly #appContext: UnknownRecord;
  readonly #runtime: ComfyVueNodeRenderRuntime;
  readonly #mounts = new Set<NativeNodeCardMount>();
  readonly #interactiveRoots = new WeakSet<HTMLElement>();

  /** Bind the installed Comfy component, application context, and renderer functions. */
  constructor(options: ComfyVueNodeCardRendererOptions) {
    this.#component = options.component;
    this.#appContext = options.appContext;
    this.#runtime = options.runtime;
  }

  /** Mount one exact internal graph node through Comfy's active native renderer. */
  mount(
    target: HTMLElement,
    node: ComfyNode,
    options: NativeNodeCardMountOptions = {},
  ): NativeNodeCardMount {
    let disposed = false;
    let advancedInputsRefreshTimer: number | null = null;
    const windowRef = target.ownerDocument.defaultView;
    const headerAccessoryHost = options.headerAccessory
      ? new ComfyVueNodeHeaderAccessoryHost(options.headerAccessory)
      : null;
    const promptTextareaScope = new CubePromptTextareaScope(target);
    const scrollBoundary = new NativeProjectedNodeScrollBoundary();
    const observer = new MutationObserver(() => {
      if (!disposed) {
        this.#applyPresentation(
          target,
          node,
          headerAccessoryHost,
          promptTextareaScope,
          collapseHost,
          scrollBoundary,
          options,
        );
      }
    });
    const renderCard = (): void => {
      if (disposed) return;
      const nodeData = createCubeFaceNodeData(this.#runtime.extractVueNodeData(node), node);
      for (const source of options.widgetSources ?? []) {
        const ownerData = createCubeFaceNodeData(
          this.#runtime.extractVueNodeData(source.node),
          source.node,
        );
        const widget = Array.isArray(ownerData.widgets)
          ? ownerData.widgets.find(
              (value: unknown) => isRecord(value) && value.name === source.name,
            )
          : undefined;
        if (isRecord(widget) && Array.isArray(nodeData.widgets)) {
          nodeData.widgets.push({ ...widget, name: source.targetName });
        }
      }
      const vnodeValue = this.#runtime.h(this.#component, { nodeData });
      if (!isRecord(vnodeValue)) {
        throw new TypeError('Comfy native renderer returned an invalid VNode.');
      }
      vnodeValue.appContext = this.#appContext;
      this.#runtime.render(vnodeValue, target);
      target.classList.add('sugarcubes-native-node-card');
      target.dataset.cubeFaceNative = 'nodes-2';
      this.#applyPresentation(
        target,
        node,
        headerAccessoryHost,
        promptTextareaScope,
        collapseHost,
        scrollBoundary,
        options,
      );
    };
    const collapseHost = new NativeProjectedNodeCollapse(target, node, renderCard);
    const refreshAfterAdvancedInputsToggle = (event: Event): void => {
      const eventTarget = event.target;
      if (!(eventTarget instanceof Element)) return;
      const toggle = eventTarget.closest('[data-testid="advanced-inputs-button"]');
      if (!toggle || !target.contains(toggle)) return;
      if (!windowRef) {
        queueMicrotask(() => queueMicrotask(renderCard));
        return;
      }
      if (advancedInputsRefreshTimer !== null) {
        windowRef.clearTimeout(advancedInputsRefreshTimer);
      }
      advancedInputsRefreshTimer = windowRef.setTimeout(() => {
        advancedInputsRefreshTimer = null;
        renderCard();
      }, 0);
    };
    renderCard();
    target.addEventListener('click', refreshAfterAdvancedInputsToggle, true);
    observer.observe(target, { childList: true, subtree: true });

    const mount: NativeNodeCardMount = {
      refresh: renderCard,
      unmount: () => {
        if (disposed) return;
        disposed = true;
        if (advancedInputsRefreshTimer !== null) {
          windowRef?.clearTimeout(advancedInputsRefreshTimer);
          advancedInputsRefreshTimer = null;
        }
        observer.disconnect();
        target.removeEventListener('click', refreshAfterAdvancedInputsToggle, true);
        target.querySelector('.lg-node')?.dispatchEvent(new Event(NATIVE_NODE_CARD_DISPOSE_EVENT));
        promptTextareaScope.dispose();
        collapseHost.dispose();
        scrollBoundary.dispose();
        headerAccessoryHost?.dispose();
        this.#runtime.render(null, target);
        this.#mounts.delete(mount);
      },
    };
    this.#mounts.add(mount);
    return mount;
  }

  /** Unmount every card created by this renderer instance. */
  dispose(): void {
    for (const mount of [...this.#mounts]) mount.unmount();
  }

  /** Apply the narrow Cube-face mode to Comfy-owned component roots. */
  #applyPresentation(
    target: HTMLElement,
    node: ComfyNode,
    headerAccessoryHost: ComfyVueNodeHeaderAccessoryHost | null,
    promptTextareaScope: CubePromptTextareaScope,
    collapseHost: NativeProjectedNodeCollapse,
    scrollBoundary: NativeProjectedNodeScrollBoundary,
    options: NativeNodeCardMountOptions,
  ): void {
    const nativeRoot = target.querySelector<HTMLElement>('.lg-node');
    if (nativeRoot) {
      if (options.presentationGeometry) {
        // Comfy's shared ResizeObserver keys canonical geometry by these attributes.
        // Presentation cards retain their native callbacks but must never write graph bounds.
        nativeRoot.dataset.presentationNodeId = String(node.id ?? '');
        nativeRoot.removeAttribute('data-node-id');
        for (const grid of nativeRoot.querySelectorAll('[data-widgets-grid-node-id]')) {
          grid.removeAttribute('data-widgets-grid-node-id');
        }
      }
      nativeRoot.dataset.cubeFacePresentation = 'true';
      for (const handle of nativeRoot.querySelectorAll<HTMLElement>(':scope > [role="button"]')) {
        handle.hidden = true;
      }
      collapseHost.reconcile(nativeRoot);
      if (node.isSubgraphNode?.()) {
        hideNativeSubgraphIcon(nativeRoot);
        hideNativeSubgraphFooter(nativeRoot);
      }
      headerAccessoryHost?.reconcile(nativeRoot, node);
    }

    const targetRecord: UnknownRecord = isRecord(target) ? target : {};
    annotateNativeBypassBadge(targetRecord._vnode, nativeRoot);
    for (const componentName of ['NodeSlots', 'NodeBadges']) {
      const vnode = findVueComponent(targetRecord._vnode, componentName);
      const element = resolveComponentElement(vnode);
      if (element) hideComponentRoot(element);
    }
    const footerVNode = findVueComponent(targetRecord._vnode, 'NodeFooter');
    const footerRoot = resolveComponentElement(footerVNode);
    const advancedInputs = footerRoot ? reconcileNativeFooter(footerRoot) : null;
    if (advancedInputs) this.#isolateInteractiveRoot(advancedInputs);
    if (nativeRoot && !options.preserveNativeBody) reconcileNativeBody(nativeRoot, node);
    promptTextareaScope.reconcile(findCubeFacePromptWidget(node) !== null);

    const widgetsVNode = findVueComponent(targetRecord._vnode, 'NodeWidgets');
    const widgetsRoot = resolveComponentElement(widgetsVNode);
    if (widgetsRoot) this.#isolateInteractiveRoot(widgetsRoot);
    scrollBoundary.reconcile(
      nativeRoot,
      [advancedInputs, widgetsRoot].filter((element): element is HTMLElement => element !== null),
    );
  }

  /** Keep embedded controls from initiating the projected native node's drag behavior. */
  #isolateInteractiveRoot(root: HTMLElement): void {
    if (this.#interactiveRoots.has(root)) return;
    const stopAtControl = (event: Event): void => event.stopPropagation();
    root.addEventListener('pointerdown', stopAtControl);
    root.addEventListener('mousedown', stopAtControl);
    root.addEventListener('contextmenu', stopAtControl);
    this.#interactiveRoots.add(root);
  }
}

/** Identify the native header's bypass badge while leaving custom and pricing badges intact. */
function annotateNativeBypassBadge(vnode: unknown, nativeRoot: HTMLElement | null): void {
  const header = findVueComponent(vnode, 'NodeHeader');
  for (const badge of findVueComponents(header, 'NodeBadge')) {
    const component = isRecord(badge.component) ? badge.component : {};
    const props = isRecord(component.props) ? component.props : {};
    const element = resolveComponentElement(badge);
    if (!element || element.closest('.lg-node') !== nativeRoot) continue;
    if (props.cssIcon === 'icon-[lucide--redo-dot]') element.dataset.masonryBypassBadge = '';
    else delete element.dataset.masonryBypassBadge;
  }
}

/** Preserve and return only the native footer control for advanced inputs. */
function reconcileNativeFooter(footerRoot: HTMLElement): HTMLElement | null {
  const advancedInputs = footerRoot.querySelector<HTMLElement>(
    '[data-testid="advanced-inputs-button"]',
  );
  if (!advancedInputs) {
    hideComponentRoot(footerRoot);
    return null;
  }
  showComponentRoot(footerRoot);
  return advancedInputs;
}

/** Remove the native body when Cube-face policy leaves only the title row. */
function reconcileNativeBody(nativeRoot: HTMLElement, node: ComfyNode): void {
  const body = nativeRoot.querySelector<HTMLElement>('[data-testid^="node-body-"]');
  if (!body) return;
  const headerOnly = !cubeFaceNodeHasVisibleWidgets(node);
  const wasHeaderOnly = nativeRoot.dataset.cubeFaceBody === 'header-only';
  if (headerOnly) {
    nativeRoot.dataset.cubeFaceBody = 'header-only';
    hideComponentRoot(body);
    return;
  }
  if (!wasHeaderOnly) return;
  delete nativeRoot.dataset.cubeFaceBody;
  body.hidden = false;
  body.style.removeProperty('display');
}

/** Hide Comfy's subgraph action footer while retaining the complete native card above it. */
function hideNativeSubgraphFooter(nativeRoot: HTMLElement): void {
  const testIdButton = nativeRoot.querySelector<HTMLElement>(
    '[data-testid="subgraph-enter-button"]',
  );
  const footer =
    testIdButton?.parentElement ??
    nativeRoot.querySelector<HTMLElement>(':scope > .isolate.-z-1.-mt-5');
  if (footer) hideComponentRoot(footer);
}

/** Hide the native subgraph glyph from one projected Cube-face card header. */
function hideNativeSubgraphIcon(nativeRoot: HTMLElement): void {
  for (const element of nativeRoot.querySelectorAll<HTMLElement>('*')) {
    if (element.classList.contains('icon-[comfy--workflow]')) hideComponentRoot(element);
  }
}

/** Hide one Comfy-owned presentation root without replacing its renderer. */
function hideComponentRoot(element: HTMLElement): void {
  element.hidden = true;
  element.style.setProperty('display', 'none', 'important');
}

/** Restore a Comfy-owned root previously hidden by Cube-face reconciliation. */
function showComponentRoot(element: HTMLElement): void {
  element.hidden = false;
  element.style.removeProperty('display');
}

/** Resolve the rendered root element for one mounted component VNode. */
function resolveComponentElement(vnode: UnknownRecord | null): HTMLElement | null {
  const component = isRecord(vnode?.component) ? vnode.component : null;
  const subTree = isRecord(component?.subTree) ? component.subTree : null;
  return subTree?.el instanceof HTMLElement ? subTree.el : null;
}
