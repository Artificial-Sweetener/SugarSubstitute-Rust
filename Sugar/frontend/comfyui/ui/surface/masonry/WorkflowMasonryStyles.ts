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
/** Own masonry layout rules and workspace stacking independently of renderer appearance. */
import { MASONRY_SURFACE_Z_INDEX } from '../../core/OverlayStacking.js';
import {
  WORKFLOW_MASONRY_INLINE_PADDING,
  WORKFLOW_MASONRY_TOP_INSET,
} from './WorkflowMasonryLayout.js';
import { packedMasonryRule } from './MasonryPacking.js';

/**
 * Marks the graph container while a masonry surface is mounted inside it.
 *
 * The rules that quieten ComfyUI's own canvas chrome under a masonry read this attribute. A rule
 * that asked the container whether it "has" a masonry instead made the browser restyle the whole
 * workspace after every change anywhere inside it.
 */
export const MASONRY_MOUNTED_ATTRIBUTE = 'data-sc-masonry-mounted';

/**
 * Marks one element whose whole subtree a masonry hides and takes out of pointer input. Its value
 * names the owner that set it.
 *
 * The attribute sits on the element that is hidden, never on the workspace container: a rule that
 * hides "everything under the container's graph layer" has to be checked against every element
 * in the container whenever the container's state changes, and the container holds every card.
 */
export const MASONRY_HIDES_ATTRIBUTE = 'data-sc-masonry-hides';
/** Marks one element whose whole subtree takes no pointer input while a masonry is mounted. */
export const MASONRY_STILLS_ATTRIBUTE = 'data-sc-masonry-stills';

const MOUNTED_HIDES = '[data-testid="selection-toolbox"]';
const MOUNTED_STILLS = '[aria-label="Canvas Toolbar"], .minimap-main-container';

/** Set or clear `attribute` on `element` only when that changes it. */
function mark(element: Element, attribute: string, value: string | null): void {
  if (element.getAttribute(attribute) === value) return;
  if (value === null) element.removeAttribute(attribute);
  else element.setAttribute(attribute, value);
}

/** Mark the suppressed graph chrome in `scope`, or clear the marks this owner set. */
function markChrome(scope: ParentNode, mounted: boolean): void {
  if (!mounted) {
    for (const element of scope.querySelectorAll(`[${MASONRY_HIDES_ATTRIBUTE}="mounted"]`))
      element.removeAttribute(MASONRY_HIDES_ATTRIBUTE);
    for (const element of scope.querySelectorAll(`[${MASONRY_STILLS_ATTRIBUTE}]`))
      element.removeAttribute(MASONRY_STILLS_ATTRIBUTE);
    return;
  }
  for (const element of scope.querySelectorAll(MOUNTED_HIDES)) markHidden(element);
  for (const element of scope.querySelectorAll(MOUNTED_STILLS))
    mark(element, MASONRY_STILLS_ATTRIBUTE, '');
}

/** Hide a selection toolbox and, where the container holds it in a wrapper, that wrapper. */
function markHidden(toolbox: Element): void {
  mark(toolbox, MASONRY_HIDES_ATTRIBUTE, 'mounted');
  const wrapper = toolbox.parentElement;
  if (wrapper?.parentElement?.id === 'graph-canvas-container')
    mark(wrapper, MASONRY_HIDES_ATTRIBUTE, 'mounted');
}

/** Watches each container with a mounted masonry for graph chrome the host mounts later. */
const lateChrome = new WeakMap<HTMLElement, MutationObserver>();

/**
 * State on `surface`'s graph container whether a masonry surface is mounted inside it, and mark
 * the graph chrome that is suppressed for as long as one is, including chrome the host mounts
 * while it is.
 */
export function markMasonryMounted(surface: HTMLElement): void {
  const root = surface.closest<HTMLElement>('#graph-canvas-container');
  if (!root) return;
  const mounted = root.querySelector('.sugarcubes-workflow-masonry') !== null;
  if (mounted !== root.hasAttribute(MASONRY_MOUNTED_ATTRIBUTE))
    root.toggleAttribute(MASONRY_MOUNTED_ATTRIBUTE, mounted);
  markChrome(root, mounted);
  const watching = lateChrome.get(root);
  if (!mounted) {
    watching?.disconnect();
    lateChrome.delete(root);
    return;
  }
  const Observer = root.ownerDocument.defaultView?.MutationObserver;
  if (watching || !Observer) return;
  const observer = new Observer((records) => {
    for (const record of records)
      for (const node of record.addedNodes) {
        if (!(node instanceof Element)) continue;
        if (node.matches(MOUNTED_HIDES)) markHidden(node);
        if (node.matches(MOUNTED_STILLS)) mark(node, MASONRY_STILLS_ATTRIBUTE, '');
        markChrome(node, true);
      }
  });
  observer.observe(root, { childList: true, subtree: true });
  lateChrome.set(root, observer);
}

/** Install layout structure independently of native node theme tokens. */
export function ensureWorkflowMasonryStyles(documentRef: Document): void {
  if (documentRef.getElementById('sugarcubes-workflow-masonry-styles')) return;
  const style = documentRef.createElement('style');
  style.id = 'sugarcubes-workflow-masonry-styles';
  style.textContent = `
    .sugarcubes-masonry-toggle {
      display:inline-flex; align-items:center; justify-content:center; flex-shrink:0;
      width:32px; height:32px; padding:0; border:0; border-radius:6px; cursor:pointer;
      background:var(--secondary-background,var(--comfy-menu-bg,#202020));
      color:var(--secondary-foreground,var(--fg-color,#ddd));
    }
    .sugarcubes-masonry-toggle:hover { background:var(--secondary-background-hover,#404040); }
    .sugarcubes-masonry-toggle:focus-visible { outline:1px solid var(--ring,currentColor); outline-offset:2px; }
    .sugarcubes-masonry-toggle:disabled { opacity:.5; cursor:default; }
    .sugarcubes-masonry-toggle__icon {
      width:20px; height:20px; pointer-events:none; background:currentColor;
      mask:var(--sc-view-icon) center / contain no-repeat;
    }
    /* Only the masonry's own padding reads the stack width; were it inherited, every change
       to it would restyle every card. */
    @property --sc-masonry-stack-width { syntax:'<length>'; inherits:false; initial-value:0px; }
    .sugarcubes-workflow-masonry {
      position:absolute; inset:0; z-index:${MASONRY_SURFACE_Z_INDEX}; box-sizing:border-box; pointer-events:auto;
      overflow-y:auto; overflow-x:hidden; overscroll-behavior:contain;
      scroll-padding-bottom:var(--sc-panel-resize-corner,0px);
      padding:${WORKFLOW_MASONRY_TOP_INSET}px var(--sc-masonry-inline-padding,${WORKFLOW_MASONRY_INLINE_PADDING}px) 96px calc(var(--sc-masonry-inline-padding,${WORKFLOW_MASONRY_INLINE_PADDING}px) + var(--sc-masonry-stack-width,0px));
      background:transparent;
      color:var(--fg-color,#ddd); outline:none;
    }
    ${packedMasonryRule('.sugarcubes-workflow-masonry')} {
      --sugarcubes-masonry-gap:0;
      --sc-masonry-inline-padding:0px;
      padding:0 0 var(--sc-panel-resize-corner,0px) var(--sc-masonry-stack-width,0px);
    }
    .sugarcubes-workflow-masonry__backdrop {
      position:absolute; inset:0; z-index:${MASONRY_SURFACE_Z_INDEX}; pointer-events:none;
      background:var(--sugarcubes-masonry-surface,var(--comfy-menu-bg,#202020));
    }
    #graph-canvas-container[data-sugarcubes-masonry="true"] {
      background:var(--sugarcubes-masonry-surface,var(--comfy-menu-bg,#202020));
    }
    .sugarcubes-workflow-masonry[data-sc-masonry-preparing] { opacity:0 !important; }
    [data-sc-masonry-source] { visibility:hidden !important; }
    .sugarcubes-workflow-masonry__card .sugarcubes-cube-face__native-card-mount {
      transform-origin:0 0;
    }
    .sugarcubes-workflow-masonry__content { position:relative; width:100%; }
    /* Retain a measurable scale box with zero flow height, anchored by native scrolling. */
    .sugarcubes-workflow-masonry > .sc-style-scrollbar-layer {
      position:sticky; inset:auto; bottom:0; width:100%; height:1px; margin-top:-1px;
    }
    .sc-plugin-overrides-slot:empty { display:none; }
    .sc-plugin-overrides-slot { position:relative; margin-bottom:var(--sugarcubes-masonry-gap,12px); }
    .sugarcubes-workflow-masonry__section {
      position:absolute; left:0; right:0; height:36px;
      display:flex; align-items:center; gap:12px;
      border-bottom:1px solid var(--interface-stroke,#454545);
    }
    .sugarcubes-workflow-masonry__section h2 {
      display:flex; align-items:center; gap:8px; min-width:0;
      font:inherit; font-weight:600; margin:0;
    }
    .sugarcubes-workflow-masonry__cube-icon {
      display:inline-flex; align-items:center; justify-content:center;
      width:24px; height:24px; flex:0 0 24px;
    }
    .sugarcubes-workflow-masonry__cube-icon img,
    .sugarcubes-workflow-masonry__cube-icon canvas { width:100%; height:100%; object-fit:contain; }
    .sugarcubes-workflow-masonry__actions { margin-left:auto; display:flex; gap:12px; align-items:center; }
    .sugarcubes-workflow-masonry__section button { color:inherit; background:transparent; border:0; cursor:pointer; }
    .sugarcubes-workflow-masonry__section .sugarcubes-workflow-masonry__icon-action {
      display:inline-flex; align-items:center; justify-content:center;
      width:28px; height:28px; padding:0; border-radius:4px;
    }
    .sugarcubes-workflow-masonry__section .sugarcubes-workflow-masonry__icon-action:hover {
      background:var(--sc-surface-hover,var(--interface-button-hover-surface,#393939));
    }
    .sugarcubes-workflow-masonry__icon-action:focus-visible {
      outline:2px solid var(--sc-brand-border,var(--primary-color,#479ef5)); outline-offset:2px;
    }
    .sugarcubes-workflow-masonry__content > .sugarcubes-workflow-masonry__card {
      position:absolute; left:0; top:0; box-sizing:border-box;
      contain:layout style;
    }
    .sugarcubes-workflow-masonry [data-sc-surface-moving],
    .sugarcubes-workflow-masonry [data-sc-surface-moving] * { pointer-events:none !important; }
    .sugarcubes-workflow-masonry__card .lg-node-content { content-visibility:visible; }
    .sugarcubes-workflow-masonry__card:has([data-reka-popper-content-wrapper]) { z-index:10; }
    #graph-canvas-container[${MASONRY_MOUNTED_ATTRIBUTE}] [data-testid="selection-toolbox"],
    #graph-canvas-container[data-sugarcubes-masonry="true"] > #graph-canvas,
    #graph-canvas-container[data-sugarcubes-masonry="true"] > .ph-no-capture,
    #graph-canvas-container[data-sugarcubes-masonry="true"] > .isolate,
    [${MASONRY_HIDES_ATTRIBUTE}],
    [${MASONRY_HIDES_ATTRIBUTE}] * {
      visibility:hidden !important; pointer-events:none !important;
    }
    #graph-canvas-container[data-sugarcubes-masonry="true"] [aria-label="Canvas Toolbar"],
    #graph-canvas-container[data-sugarcubes-masonry="true"] .minimap-main-container {
      visibility:hidden !important;
    }
    #graph-canvas-container[${MASONRY_MOUNTED_ATTRIBUTE}] [aria-label="Canvas Toolbar"],
    #graph-canvas-container[${MASONRY_MOUNTED_ATTRIBUTE}] .minimap-main-container,
    [${MASONRY_STILLS_ATTRIBUTE}],
    [${MASONRY_STILLS_ATTRIBUTE}] * {
      pointer-events:none !important;
    }
    .sugarcubes-workflow-masonry__empty { padding:32px; opacity:.7; }
  `;
  documentRef.head.append(style);
}
