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
import { isGraphChromeLayer } from '../../core/ComfyGraphChromeLayer.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { MASONRY_SURFACE_Z_INDEX } from '../../core/OverlayStacking.js';
import {
  WORKFLOW_MASONRY_INLINE_PADDING,
  WORKFLOW_MASONRY_TOP_INSET,
} from './WorkflowMasonryLayout.js';
import { packedMasonryRule } from './MasonryPacking.js';

/**
 * The gap an expanded cube stack leaves before the cards: the masonry's own card gap. The gap
 * token is a unitless pixel count (JavaScript reads it as a number), so it becomes a length here.
 */
export const MASONRY_STACK_GAP = 'calc(var(--sugarcubes-masonry-gap,8) * 1px)';
/**
 * Marks one element whose whole subtree a masonry hides and takes out of pointer input. Its value
 * names the owner that set it.
 *
 * The attribute sits on the element that is hidden, never on the workspace container: a rule that
 * reaches from the container's state to something inside it has to be checked against every
 * element in the container whenever that state changes, and the container holds every card.
 */
export const MASONRY_HIDES_ATTRIBUTE = 'data-sc-masonry-hides';
/** Marks one element whose whole subtree takes no pointer input while a masonry is mounted. */
export const MASONRY_STILLS_ATTRIBUTE = 'data-sc-masonry-stills';

const MOUNTED_HIDES = '[data-testid="selection-toolbox"]';
/** The graph's own navigation chrome: stilled under a mounted masonry, hidden once it rests. */
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
const lateChrome = new WeakMap<HTMLElement, HostSurfaceObserver>();

/**
 * Mark the graph chrome in `surface`'s graph container that is suppressed for as long as a
 * masonry surface is mounted inside it, including chrome the host mounts while one is.
 */
export function markMasonryMounted(surface: HTMLElement): void {
  const root = surface.closest<HTMLElement>('#graph-canvas-container');
  if (!root) return;
  const mounted = root.querySelector('.sugarcubes-workflow-masonry') !== null;
  markChrome(root, mounted);
  const watching = lateChrome.get(root);
  if (!mounted) {
    watching?.disconnect();
    lateChrome.delete(root);
    return;
  }
  if (watching) return;
  lateChrome.set(
    root,
    new HostSurfaceObserver(
      root,
      `${MOUNTED_HIDES}, ${MOUNTED_STILLS}`,
      (changes) => {
        for (const change of changes) {
          if (change.kind !== 'mounted') continue;
          if (change.element.matches(MOUNTED_HIDES)) markHidden(change.element);
          if (change.element.matches(MOUNTED_STILLS))
            mark(change.element, MASONRY_STILLS_ATTRIBUTE, '');
        }
      },
      { exclude: '.sugarcubes-workflow-masonry' },
    ),
  );
}

/** Watches each container whose graph is hidden for anything mounted into it later. */
const lateGraphChildren = new WeakMap<HTMLElement, HostSurfaceObserver>();

/**
 * Whether SugarCubes mounted or adopted `element`: its own surfaces carry an `sc-` or
 * `sugarcubes-` class, and host elements it has taken into its frame carry a `data-sc-` mark.
 */
function ownedSurface(element: Element): boolean {
  for (const name of element.classList)
    if (name.startsWith('sc-') || name.startsWith('sugarcubes-')) return true;
  for (const attribute of element.attributes)
    if (
      attribute.name.startsWith('data-sc-') &&
      attribute.name !== MASONRY_HIDES_ATTRIBUTE &&
      attribute.name !== MASONRY_STILLS_ATTRIBUTE
    )
      return true;
  return false;
}

/**
 * Hide everything in the graph container `root` that is not SugarCubes' own, for as long as its
 * masonry is shown; or show again what this hid.
 *
 * The masonry lets the desktop's backdrop show through, so anything left visible in the
 * container shows behind the cards. The rule is by ownership, not by a list of known graph
 * layers: a canvas, an overlay, a preview or a toolbar the host or another extension mounts,
 * now or later, is hidden because it is not ours. `workspace` is where the masonry mounts.
 */
export function markMasonryGraphHidden(
  root: HTMLElement,
  hidden: boolean,
  workspace: HTMLElement | null = null,
): void {
  // The host element the masonry is mounted in is kept, whoever owns it, and so is ComfyUI's
  // application chrome layer: the graph navigation chrome inside either is hidden by the rules
  // for that chrome.
  const kept = (element: Element): boolean =>
    ownedSurface(element) ||
    isGraphChromeLayer(element) ||
    (workspace !== null && (element === workspace || element.contains(workspace))) ||
    element.querySelector('.sugarcubes-workflow-masonry') !== null;
  // Whether a child is kept can change after it mounts (the chrome layer gains its panel, a
  // workspace moves), so every pass reconsiders every child: marks are set and cleared, never
  // only added. The container holds a handful of children, so a pass is cheap.
  const reconcile = (): void => {
    for (const element of root.children) {
      const current = element.getAttribute(MASONRY_HIDES_ATTRIBUTE);
      if (kept(element)) {
        if (current === 'graph') element.removeAttribute(MASONRY_HIDES_ATTRIBUTE);
      } else if (current === null) mark(element, MASONRY_HIDES_ATTRIBUTE, 'graph');
    }
  };
  const watching = lateGraphChildren.get(root);
  if (!hidden) {
    watching?.disconnect();
    lateGraphChildren.delete(root);
    for (const element of root.querySelectorAll(`[${MASONRY_HIDES_ATTRIBUTE}="graph"]`))
      element.removeAttribute(MASONRY_HIDES_ATTRIBUTE);
    return;
  }
  reconcile();
  if (watching) return;
  lateGraphChildren.set(
    root,
    new HostSurfaceObserver(
      root,
      '#graph-canvas-container > *, .graph-canvas-panel',
      (changes) => {
        if (changes.some((change) => change.kind !== 'content')) reconcile();
      },
      { exclude: '.sugarcubes-workflow-masonry' },
    ),
  );
}

/** Watches each container whose graph is hidden for navigation chrome the host mounts later. */
const lateHiddenChrome = new WeakMap<HTMLElement, HostSurfaceObserver>();

/**
 * Hide the graph's navigation chrome in `root` for as long as its graph is hidden, including
 * chrome the host mounts meanwhile; or show again what this hid.
 */
export function markMasonryGraphChromeHidden(root: HTMLElement, hidden: boolean): void {
  const watching = lateHiddenChrome.get(root);
  if (!hidden) {
    watching?.disconnect();
    lateHiddenChrome.delete(root);
    for (const element of root.querySelectorAll(`[${MASONRY_HIDES_ATTRIBUTE}="graph-chrome"]`))
      element.removeAttribute(MASONRY_HIDES_ATTRIBUTE);
    return;
  }
  for (const element of root.querySelectorAll(MOUNTED_STILLS))
    mark(element, MASONRY_HIDES_ATTRIBUTE, 'graph-chrome');
  if (watching) return;
  lateHiddenChrome.set(
    root,
    new HostSurfaceObserver(
      root,
      MOUNTED_STILLS,
      (changes) => {
        for (const change of changes)
          if (change.kind === 'mounted')
            mark(change.element, MASONRY_HIDES_ATTRIBUTE, 'graph-chrome');
      },
      { exclude: '.sugarcubes-workflow-masonry' },
    ),
  );
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
      position:absolute; inset:0 0 var(--sc-console-inset,0px) 0; z-index:${MASONRY_SURFACE_Z_INDEX}; box-sizing:border-box; pointer-events:auto;
      overflow-y:auto; overflow-x:hidden; overscroll-behavior:contain;
      scroll-padding-bottom:var(--sc-panel-resize-corner,0px);
      padding:${WORKFLOW_MASONRY_TOP_INSET}px var(--sc-masonry-inline-padding,${WORKFLOW_MASONRY_INLINE_PADDING}px) 96px calc(var(--sc-masonry-stack-gap,var(--sc-masonry-inline-padding,${WORKFLOW_MASONRY_INLINE_PADDING}px)) + var(--sc-masonry-stack-width,0px));
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
    [data-sc-masonry-source] { visibility:hidden !important; opacity:0 !important; }
    .sugarcubes-workflow-masonry__card .sugarcubes-cube-face__native-card-mount {
      transform-origin:0 0;
    }
    .sugarcubes-workflow-masonry__content { position:relative; width:100%; }
    .sugarcubes-workflow-masonry__tail { display:none !important; }
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
    /* The actions are the active style's own buttons; this only places them in the header. */
    .sugarcubes-workflow-masonry__actions {
      margin-left:auto; display:flex; gap:8px; align-items:center; padding-inline-end:12px;
    }
    .sugarcubes-workflow-masonry__actions > [data-sc-button] { flex:none; }
    /* Comfy's own button face, from Comfy's tokens, for the native button its style is given:
       frameless at rest, its frame and fill shown on hover. */
    html[data-sc-visual-style="comfy"] .sugarcubes-workflow-masonry__actions > button[data-sc-button] {
      display:inline-flex; align-items:center; justify-content:center; box-sizing:border-box;
      width:32px; height:32px; padding:0; border-radius:8px; cursor:pointer;
      border:1px solid transparent; background:transparent;
      color:var(--base-foreground,var(--fg-color,#eee));
    }
    html[data-sc-visual-style="comfy"] .sugarcubes-workflow-masonry__actions > button[data-sc-button]:hover {
      border-color:var(--border-subtle,var(--border-color,#4e4e4e));
      background:var(--secondary-background-hover,#393939);
    }
    html[data-sc-visual-style="comfy"] .sugarcubes-workflow-masonry__actions > button[data-sc-button]:focus-visible {
      outline:2px solid var(--primary-background,#0b8ce9); outline-offset:1px;
    }
    .sugarcubes-workflow-masonry__content > .sugarcubes-workflow-masonry__card {
      position:absolute; left:0; top:0; box-sizing:border-box;
      contain:layout style;
    }
    .sugarcubes-workflow-masonry [data-sc-surface-moving],
    .sugarcubes-workflow-masonry [data-sc-surface-moving] * { pointer-events:none !important; }
    .sugarcubes-workflow-masonry__card .lg-node-content { content-visibility:visible; }
    .sugarcubes-workflow-masonry__card:has([data-reka-popper-content-wrapper]) { z-index:10; }
    [${MASONRY_HIDES_ATTRIBUTE}],
    [${MASONRY_HIDES_ATTRIBUTE}] * {
      visibility:hidden !important; pointer-events:none !important;
    }
    /* A descendant can declare itself visible again; nothing inside can undo its ancestor's opacity. */
    [${MASONRY_HIDES_ATTRIBUTE}] { opacity:0 !important; }
    [${MASONRY_STILLS_ATTRIBUTE}],
    [${MASONRY_STILLS_ATTRIBUTE}] * {
      pointer-events:none !important;
    }
    .sugarcubes-workflow-masonry__empty { padding:32px; opacity:.7; }
  `;
  documentRef.head.append(style);
}
