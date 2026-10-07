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

import { scopeNativeVueMountContext } from './NativeVueMountContext.js';
import { isNativeWidgetPresented } from './NativeWidgetPresentation.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';
import type { ComfyVueNodeRenderRuntime } from './ComfyRuntimeModuleLoader.js';
import { findVueComponent, findVueComponents } from './ComfyVueTree.js';
import {
  createCubeFaceNodeData,
  cubeFaceNodeHasPresentedWidgets,
  cubeFaceNodeHasVisibleWidgets,
} from './CubeFaceNodePresentationPolicy.js';
import { findCubeFacePromptWidget } from './CubeFacePromptPolicy.js';
import { CubePromptTextareaScope } from './CubePromptTextareaScope.js';
import { ComfyVueNodeHeaderAccessoryHost } from './ComfyVueNodeHeaderAccessoryHost.js';
import { NativeProjectedNodeCollapse } from './NativeProjectedNodeCollapse.js';
import { CardDisclosureMotion } from './motion/CardDisclosureMotion.js';
import { NativeCardContentDisclosure } from './NativeCardContentDisclosure.js';
import { NativeProjectedAdvancedInputs } from './NativeProjectedAdvancedInputs.js';
import { NativeProjectedNodeScrollBoundary } from './NativeProjectedNodeScrollBoundary.js';
import { NativeProjectedNodeTitleBoundary } from './NativeProjectedNodeTitleBoundary.js';
import { NATIVE_NODE_CARD_DISPOSE_EVENT } from './NativeNodeCardRenderer.js';
import { subscribeNativeNodeCardRefresh } from './NativeNodeCardRefresh.js';
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

  /** Resynchronize native socket caches after presentation styles or borrowed controls settle. */
  requestSlotLayoutSync(): void {
    this.#runtime.requestSlotLayoutSync?.();
  }

  /** Mount one exact internal graph node through Comfy's active native renderer. */
  mount(
    target: HTMLElement,
    node: ComfyNode,
    options: NativeNodeCardMountOptions = {},
  ): NativeNodeCardMount {
    let disposed = false;
    const scopedContext = scopeNativeVueMountContext(this.#appContext);
    const headerAccessoryHost = options.headerAccessory
      ? new ComfyVueNodeHeaderAccessoryHost(options.headerAccessory)
      : null;
    const promptTextareaScope = new CubePromptTextareaScope(target);
    const scrollBoundary = new NativeProjectedNodeScrollBoundary();
    const titleBoundary = new NativeProjectedNodeTitleBoundary(
      target,
      options.allowRename !== false,
    );
    const disclosureMotion = new CardDisclosureMotion(target);
    const contentDisclosure = new NativeCardContentDisclosure(target, disclosureMotion);
    const observer = new MutationObserver(() => {
      if (!disposed) {
        this.#applyPresentation(
          target,
          node,
          headerAccessoryHost,
          promptTextareaScope,
          collapseHost,
          advancedInputsHost,
          scrollBoundary,
          options,
        );
      }
    });
    const renderCard = (): void => {
      if (disposed) return;
      const nodeData = createCubeFaceNodeData(
        this.#runtime.extractVueNodeData(node),
        node,
        renderCard,
        options.allowCollapse,
      );
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
      // Promoted editor controls can be absent from Comfy's extracted Vue widget list.
      const widgets = [nodeData.widgets, nodeData.sugarcubesEditorWidgets].flatMap(
        (value: unknown) => (Array.isArray(value) ? value : []),
      );
      contentDisclosure.reconcile(
        [
          ...new Set(
            widgets.flatMap((widget: unknown) =>
              isRecord(widget) && typeof widget.name === 'string' ? [widget.name] : [],
            ),
          ),
        ].sort(),
        () => {
          // Comfy snapshots advanced footer state at setup instead of watching native node changes.
          const vnodeValue = this.#runtime.h(this.#component, {
            nodeData,
            key: node.showAdvanced === true,
          });
          if (!isRecord(vnodeValue)) {
            throw new TypeError('Comfy native renderer returned an invalid VNode.');
          }
          vnodeValue.appContext = scopedContext;
          this.#runtime.render(vnodeValue, target);
          if (!target.classList.contains('sugarcubes-native-node-card'))
            target.classList.add('sugarcubes-native-node-card');
          if (target.dataset.cubeFaceNative !== 'nodes-2')
            target.dataset.cubeFaceNative = 'nodes-2';
          this.#applyPresentation(
            target,
            node,
            headerAccessoryHost,
            promptTextareaScope,
            collapseHost,
            advancedInputsHost,
            scrollBoundary,
            options,
          );
        },
      );
    };
    const collapseHost = new NativeProjectedNodeCollapse(
      target,
      node,
      renderCard,
      disclosureMotion,
    );
    const advancedInputsHost = new NativeProjectedAdvancedInputs(target, node, renderCard);
    renderCard();
    const releaseNativeRefresh = subscribeNativeNodeCardRefresh(
      target.ownerDocument,
      [node, ...(options.widgetSources ?? []).map((source) => source.node)],
      renderCard,
    );
    observer.observe(target, { childList: true, subtree: true });

    const mount: NativeNodeCardMount = {
      refresh: renderCard,
      unmount: () => {
        if (disposed) return;
        disposed = true;
        observer.disconnect();
        releaseNativeRefresh();
        target.querySelector('.lg-node')?.dispatchEvent(new Event(NATIVE_NODE_CARD_DISPOSE_EVENT));
        promptTextareaScope.dispose();
        collapseHost.dispose();
        disclosureMotion.dispose();
        advancedInputsHost.dispose();
        scrollBoundary.dispose();
        titleBoundary.dispose();
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
    advancedInputsHost: NativeProjectedAdvancedInputs,
    scrollBoundary: NativeProjectedNodeScrollBoundary,
    options: NativeNodeCardMountOptions,
  ): void {
    const nativeRoot = target.querySelector<HTMLElement>('.lg-node');
    const forwardedPresented =
      options.widgetSources?.some((source) =>
        source.node.widgets?.some(
          (widget) => widget.name === source.name && isNativeWidgetPresented(widget),
        ),
      ) ?? false;
    const hasPresented = cubeFaceNodeHasPresentedWidgets(node) || forwardedPresented;
    if (nativeRoot) {
      if (options.presentationGeometry) {
        // Comfy's shared ResizeObserver keys canonical geometry by these attributes.
        // Presentation cards retain their native callbacks but must never write graph bounds.
        const identity = String(node.id ?? '');
        if (nativeRoot.dataset.presentationNodeId !== identity)
          nativeRoot.dataset.presentationNodeId = identity;
        nativeRoot.removeAttribute('data-node-id');
        for (const grid of nativeRoot.querySelectorAll('[data-widgets-grid-node-id]')) {
          grid.removeAttribute('data-widgets-grid-node-id');
        }
      }
      if (nativeRoot.dataset.cubeFacePresentation !== 'true')
        nativeRoot.dataset.cubeFacePresentation = 'true';
      for (const handle of nativeRoot.querySelectorAll<HTMLElement>(':scope > [role="button"]')) {
        if (!handle.hidden) handle.hidden = true;
      }
      collapseHost.reconcile(
        nativeRoot,
        options.allowCollapse === false ? false : forwardedPresented ? true : undefined,
      );
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
    if (footerRoot) {
      advancedInputsHost.reconcile(footerRoot);
      if (!footerRoot.hidden) this.#isolateInteractiveRoot(footerRoot);
    }
    if (
      nativeRoot &&
      (!options.preserveNativeBody ||
        cubeFaceNodeHasVisibleWidgets(node) ||
        (options.widgetSources?.length ?? 0) > 0 ||
        nativeRoot.dataset.cubeFaceBody)
    )
      reconcileNativeBody(nativeRoot, hasPresented);
    promptTextareaScope.reconcile(findCubeFacePromptWidget(node) !== null);

    const widgetsVNode = findVueComponent(targetRecord._vnode, 'NodeWidgets');
    const widgetsRoot = resolveComponentElement(widgetsVNode);
    if (widgetsRoot) this.#isolateInteractiveRoot(widgetsRoot);
    scrollBoundary.reconcile(
      nativeRoot,
      [widgetsRoot, footerRoot?.hidden ? null : footerRoot].filter(
        (element): element is HTMLElement => element instanceof HTMLElement,
      ),
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
    if (props.cssIcon === 'icon-[lucide--redo-dot]') {
      if (!element.hasAttribute('data-masonry-bypass-badge'))
        element.dataset.masonryBypassBadge = '';
    } else delete element.dataset.masonryBypassBadge;
  }
}

/** Remove the native body when Cube-face policy leaves only the title row. */
function reconcileNativeBody(nativeRoot: HTMLElement, hasPresented: boolean): void {
  const body = nativeRoot.querySelector<HTMLElement>('[data-testid^="node-body-"]');
  if (!body) return;
  const headerOnly = !hasPresented;
  const wasHeaderOnly = nativeRoot.dataset.cubeFaceBody === 'header-only';
  if (headerOnly) {
    if (!wasHeaderOnly) nativeRoot.dataset.cubeFaceBody = 'header-only';
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
  if (!element.hidden) element.hidden = true;
  if (
    element.style.display !== 'none' ||
    element.style.getPropertyPriority('display') !== 'important'
  )
    element.style.setProperty('display', 'none', 'important');
}

/** Resolve the rendered root element for one mounted component VNode. */
function resolveComponentElement(vnode: UnknownRecord | null): HTMLElement | null {
  const component = isRecord(vnode?.component) ? vnode.component : null;
  const subTree = isRecord(component?.subTree) ? component.subTree : null;
  return subTree?.el instanceof HTMLElement ? subTree.el : null;
}
