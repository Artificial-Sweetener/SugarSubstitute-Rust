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
/** Own global discovery and the reversible lifetime of themed overlay scrollbars. */
import { nativeScrollbarCss, scrollbarPresentationCss } from './ScrollbarStyles.js';
import { scrollbarTrack } from './ScrollbarLayout.js';
import { ScrollbarView } from './ScrollbarView.js';
import { ScrollbarLayers, scrollbarCoordinateSpace } from './ScrollbarLayers.js';
import type { ScrollbarCoordinateSpace } from './ScrollbarLayers.js';
import { SURFACE_GEOMETRY_MOTION_EVENT } from '../motion/SurfaceGeometryMotion.js';

interface ScrollOwner {
  vertical: ScrollbarView;
  horizontal: ScrollbarView;
  assignedId: string | null;
}

/** Observe native overflow while leaving the host's layout and scrolling intact. */
export class OverlayScrollbarHost {
  readonly #document: Document;
  readonly #reportError: (error: unknown) => void;
  readonly #layers: ScrollbarLayers;
  readonly #owners = new Map<HTMLElement, ScrollOwner>();
  readonly #views = new Map<HTMLElement, ScrollbarView>();
  readonly #roots = new Set<Node>();
  readonly #shadowStyles = new Map<ShadowRoot, HTMLStyleElement>();
  readonly #pending = new Set<HTMLElement>();
  readonly #changedElements = new Set<HTMLElement>();
  readonly #dirtyOwners = new Set<HTMLElement>();
  readonly #observedSizes = new Set<HTMLElement>();
  readonly #movingRegions = new Set<HTMLElement>();
  readonly #mutations: MutationObserver;
  readonly #resize: ResizeObserver | null;
  #active = false;
  #frame: number | null = null;
  #nextId = 0;

  /** Prepare native surface layers and coalesced observers. */
  constructor(document: Document, reportError: (error: unknown) => void) {
    this.#document = document;
    this.#reportError = reportError;
    this.#layers = new ScrollbarLayers(document);
    this.#mutations = new MutationObserver((records) => {
      let removed = false;
      for (const record of records) {
        if (
          record.target instanceof HTMLElement &&
          !this.#owned(record.target) &&
          (record.type !== 'childList' || record.target !== this.#document.body)
        )
          this.#changedElements.add(record.target);
        for (const node of record.addedNodes)
          if (node instanceof HTMLElement && !this.#owned(node)) this.#pending.add(node);
        for (const node of record.removedNodes)
          if (
            node instanceof HTMLElement &&
            !this.#owned(node) &&
            !node.matches('.sc-style-scrollbar,.sc-style-scrollbar-layer')
          )
            removed = true;
      }
      if (removed || this.#pending.size || this.#changedElements.size) this.#schedule();
    });
    this.#resize =
      typeof ResizeObserver === 'function'
        ? new ResizeObserver((entries) => {
            for (const entry of entries)
              if (entry.target instanceof HTMLElement) this.#invalidateGeometry(entry.target);
            this.#schedule();
          })
        : null;
  }

  /** Start the policy once, including existing host dialogs and open shadow surfaces. */
  enable(): void {
    if (this.#active) return;
    this.#active = true;
    this.#observe(this.#document.body);
    this.#pending.add(this.#document.body);
    this.#document.addEventListener('scroll', this.#onContentChange, true);
    this.#document.addEventListener('input', this.#onContentChange, true);
    this.#document.addEventListener(SURFACE_GEOMETRY_MOTION_EVENT, this.#onGeometryMotion);
    this.#document.defaultView?.addEventListener('resize', this.#onViewportResize);
    this.#document.defaultView?.addEventListener('pointerdown', this.#captureThumb, true);
    this.#schedule();
  }

  /** Restore native presentation and remove only IDs and styles owned by this host. */
  clear(): void {
    this.#active = false;
    if (this.#frame !== null) this.#document.defaultView?.cancelAnimationFrame(this.#frame);
    this.#frame = null;
    this.#mutations.disconnect();
    this.#resize?.disconnect();
    this.#observedSizes.clear();
    this.#document.removeEventListener('scroll', this.#onContentChange, true);
    this.#document.removeEventListener('input', this.#onContentChange, true);
    this.#document.removeEventListener(SURFACE_GEOMETRY_MOTION_EVENT, this.#onGeometryMotion);
    this.#movingRegions.clear();
    this.#document.defaultView?.removeEventListener('resize', this.#onViewportResize);
    this.#document.defaultView?.removeEventListener('pointerdown', this.#captureThumb, true);
    for (const [element, owner] of this.#owners) this.#remove(element, owner);
    this.#owners.clear();
    this.#views.clear();
    for (const [root, style] of this.#shadowStyles) {
      style.remove();
      root.removeEventListener('scroll', this.#onContentChange, true);
    }
    this.#shadowStyles.clear();
    this.#roots.clear();
    this.#pending.clear();
    this.#changedElements.clear();
    this.#dirtyOwners.clear();
    this.#layers.clear();
  }

  /** Exclude owned layers and policy styles from rediscovery. */
  #owned(element: HTMLElement): boolean {
    return (
      !!element.closest('.sc-style-scrollbar-layer') ||
      element.hasAttribute('data-sc-scrollbar-policy') ||
      element.hasAttribute('data-sc-scrollbar-managed')
    );
  }

  /** Subscribe once per light or open shadow tree. */
  #observe(root: Node): void {
    if (this.#roots.has(root)) return;
    this.#roots.add(root);
    this.#mutations.observe(root, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['class', 'style', 'hidden'],
    });
  }

  /** Batch overflow reads across unique changed subtrees before inserting tracks or shadow styles. */
  #discover(roots: readonly HTMLElement[], changed: readonly HTMLElement[]): void {
    const elements = new Set(changed.filter((element) => element.isConnected));
    const pending = [...roots];
    const scanned = new Set<HTMLElement>();
    while (pending.length) {
      const root = pending.pop();
      if (!root?.isConnected || this.#owned(root) || scanned.has(root)) continue;
      scanned.add(root);
      const subtree = [root, ...root.querySelectorAll<HTMLElement>('*')];
      for (const element of subtree) elements.add(element);
      for (const element of subtree)
        if (element.shadowRoot)
          pending.push(...element.shadowRoot.querySelectorAll<HTMLElement>('*'));
    }
    const scrolling = this.#document.scrollingElement;
    if (scrolling instanceof HTMLElement) elements.add(scrolling);
    const candidates = [...elements]
      .filter((element) => !this.#owned(element))
      .map((element) => {
        const style = this.#document.defaultView?.getComputedStyle(element);
        return {
          element,
          scrollable:
            element === scrolling ||
            element.tagName === 'TEXTAREA' ||
            /auto|scroll/.test(`${style?.overflow} ${style?.overflowX} ${style?.overflowY}`),
        };
      });
    for (const { element, scrollable } of candidates) this.#consider(element, scrollable);
  }

  /** Register potential overflow even before asynchronously loaded content fills it. */
  #consider(element: HTMLElement, scrollable: boolean): void {
    if (scrollable && !this.#owners.has(element)) {
      const assignedId = element.id ? null : `sc-style-scroll-owner-${++this.#nextId}`;
      if (assignedId) element.id = assignedId;
      const update = (): void => {
        this.#invalidateGeometry(element);
        this.#schedule();
      };
      const vertical = new ScrollbarView(element, 'vertical', element.id, update);
      const horizontal = new ScrollbarView(element, 'horizontal', element.id, update);
      this.#owners.set(element, { vertical, horizontal, assignedId });
      this.#dirtyOwners.add(element);
      this.#views.set(vertical.element, vertical);
      this.#views.set(horizontal.element, horizontal);
      this.#observeSize(element);
    }
    if (scrollable)
      for (const child of element.children)
        if (child instanceof HTMLElement) this.#observeSize(child);
    const root = element.shadowRoot;
    if (root && !this.#shadowStyles.has(root)) {
      const styleElement = this.#document.createElement('style');
      styleElement.setAttribute('data-sc-scrollbar-policy', '');
      styleElement.textContent = nativeScrollbarCss() + scrollbarPresentationCss();
      root.append(styleElement);
      this.#shadowStyles.set(root, styleElement);
      root.addEventListener('scroll', this.#onContentChange, true);
      this.#observe(root);
    }
  }

  /** Tear down a detached owner without restoring IDs subsequently changed by the host. */
  #remove(element: HTMLElement, owner: ScrollOwner): void {
    this.#dirtyOwners.delete(element);
    this.#views.delete(owner.vertical.element);
    this.#views.delete(owner.horizontal.element);
    owner.vertical.dispose();
    owner.horizontal.dispose();
    this.#layers.release(element);
    this.#resize?.unobserve(element);
    this.#observedSizes.delete(element);
    for (const child of element.children)
      if (child instanceof HTMLElement) this.#resize?.unobserve(child);
    if (owner.assignedId && element.id === owner.assignedId) element.removeAttribute('id');
  }

  /** Route owned thumb presses before document-level popup dismissal and graph handlers. */
  readonly #captureThumb = (event: PointerEvent): void => {
    const thumb = event
      .composedPath()
      .find((node) => node instanceof HTMLElement && this.#views.has(node));
    const view = thumb instanceof HTMLElement ? this.#views.get(thumb) : undefined;
    if (!view || event.button !== 0) return;
    view.beginDrag(event);
    event.stopImmediatePropagation();
  };

  /** Subscribe once per size owner so repeated discovery cannot retrigger resize delivery. */
  #observeSize(element: HTMLElement): void {
    if (!this.#resize || this.#observedSizes.has(element)) return;
    this.#observedSizes.add(element);
    this.#resize.observe(element);
  }

  /** Release removed content and shadow trees instead of retaining them until theme shutdown. */
  #prune(): void {
    for (const element of this.#observedSizes) {
      if (element.isConnected) continue;
      this.#resize?.unobserve(element);
      this.#observedSizes.delete(element);
    }
    let rootsChanged = false;
    for (const [root, style] of this.#shadowStyles) {
      if (root.host.isConnected) continue;
      style.remove();
      root.removeEventListener('scroll', this.#onContentChange, true);
      this.#shadowStyles.delete(root);
      this.#roots.delete(root);
      rootsChanged = true;
    }
    if (rootsChanged) {
      this.#mutations.disconnect();
      const roots = [...this.#roots];
      this.#roots.clear();
      for (const root of roots) this.#observe(root);
    }
  }

  /** Refresh the affected scroll container and tracks whose viewport position it moves. */
  #invalidateGeometry(element: HTMLElement): void {
    for (const owner of this.#owners.keys())
      if (owner === element || owner.contains(element) || element.contains(owner))
        this.#dirtyOwners.add(owner);
  }

  /** Defer only observers intersecting an identified moving surface, retaining unrelated controls. */
  #moving(element: HTMLElement): boolean {
    return [...this.#movingRegions].some(
      (region) => region.contains(element) || element.contains(region),
    );
  }

  /** Hide changing tracks and flush their geometry once the presentation owner settles. */
  readonly #onGeometryMotion = (event: Event): void => {
    if (!(event.target instanceof HTMLElement)) return;
    const region = event.target;
    if (region.hasAttribute('data-sc-geometry-moving')) this.#movingRegions.add(region);
    else this.#movingRegions.delete(region);
    this.#invalidateGeometry(region);
    for (const [element, owner] of this.#owners)
      if (this.#moving(element)) {
        owner.vertical.hide();
        owner.horizontal.hide();
      }
    this.#schedule();
  };

  /** Keep input and scrolling local to the event's native container. */
  readonly #onContentChange = (event: Event): void => {
    const target = event.composedPath().find((node) => node instanceof HTMLElement);
    if (target instanceof HTMLElement) this.#invalidateGeometry(target);
    this.#schedule();
  };

  /** Reposition every viewport track only when the viewport itself changes. */
  readonly #onViewportResize = (): void => {
    for (const owner of this.#owners.keys()) this.#dirtyOwners.add(owner);
    this.#schedule();
  };

  /** Batch geometry reads and fail visibly through the existing diagnostic owner. */
  readonly #schedule = (): void => {
    if (!this.#active || this.#frame !== null) return;
    const run = (): void => {
      this.#frame = null;
      if (!this.#active) return;
      try {
        this.#prune();
        const pending = [...this.#pending].filter((element) => !this.#moving(element));
        for (const element of pending) this.#pending.delete(element);
        const changed = [...this.#changedElements].filter((element) => !this.#moving(element));
        for (const element of changed) this.#changedElements.delete(element);
        this.#discover(pending, changed);
        for (const element of changed)
          if (element.isConnected) {
            this.#invalidateGeometry(element);
          }
        const spaces = new Map<HTMLElement, ScrollbarCoordinateSpace>();
        const dirty: Array<{ owner: HTMLElement; vertical: HTMLElement; horizontal: HTMLElement }> =
          [];
        for (const [element, owner] of this.#owners) {
          if (!element.isConnected) {
            this.#remove(element, owner);
            this.#owners.delete(element);
            continue;
          }
          if (!this.#dirtyOwners.has(element) || this.#moving(element)) continue;
          dirty.push({
            owner: element,
            vertical: owner.vertical.element,
            horizontal: owner.horizontal.element,
          });
        }
        const layers = this.#layers.mountMany(dirty);
        const writes: Array<() => void> = [];
        for (const { owner: element } of dirty) {
          const owner = this.#owners.get(element);
          const layer = layers.get(element);
          if (!owner || !layer) continue;
          let space = spaces.get(layer);
          if (!space) {
            space = scrollbarCoordinateSpace(layer);
            spaces.set(layer, space);
          }
          const track = scrollbarTrack(element);
          writes.push(owner.vertical.prepare(track, space), owner.horizontal.prepare(track, space));
          this.#dirtyOwners.delete(element);
        }
        for (const write of writes) write();
      } catch (error: unknown) {
        this.#reportError(error);
        this.clear();
      }
    };
    const windowRef = this.#document.defaultView;
    if (windowRef?.requestAnimationFrame) this.#frame = windowRef.requestAnimationFrame(run);
    else {
      this.#frame = -1;
      queueMicrotask(run);
    }
  };
}
