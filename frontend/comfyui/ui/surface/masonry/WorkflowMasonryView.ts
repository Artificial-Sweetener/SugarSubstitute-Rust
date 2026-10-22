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
/** Own the continuous scroll surface and measured native card placement. */
import { WorkflowMasonryRemountQueue } from './WorkflowMasonryRemountQueue.js';
import type { CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import type { CubeFaceCardStateService } from '../CubeFaceCardStateService.js';
import type { NativeNodeCardRenderer } from '../NativeNodeCardRenderer.js';
import { NativeCardGeometryObserver } from '../NativeCardGeometryObserver.js';
import { ensureCubeSurfaceStyles } from '../CubeSurfaceStyles.js';
import type { CubeMasonryOptions } from '../CubeMasonryLayout.js';
import { computeWorkflowMasonry } from './WorkflowMasonryLayout.js';
import { previewMasonrySections } from './WorkflowMasonryPreview.js';
import type { WorkflowMasonrySection } from './WorkflowMasonryProjection.js';
import {
  WorkflowMasonrySectionView,
  type WorkflowMasonryPendingMount,
} from './WorkflowMasonrySectionView.js';
import { SurfaceMotion, type SurfacePlacement } from '../motion/SurfaceMotion.js';
import { MasonryStackInsetMotion } from './MasonryStackInsetMotion.js';
import { SurfacePresentationBoundary } from '../motion/SurfacePresentationBoundary.js';
import { prepareNativePresentation } from '../NativePresentationReady.js';
import { COMPACT_MASONRY_CHANGE } from './CompactMasonryPreference.js';
import {
  measureNativeCardPresentationSources,
  restoreNativeCardPresentations,
} from '../NativeCardPresentationOwner.js';
import type { WorkflowMasonryTransitionCard } from './WorkflowMasonryTransition.js';
import {
  DISCLOSURE_LAYOUT_EVENT,
  measureDisclosureCardHeight,
} from '../motion/DisclosureLayout.js';

export interface WorkflowMasonryViewOptions {
  document: Document;
  sections: readonly WorkflowMasonrySection[];
  cardState: CubeFaceCardStateService;
  openCube(node: CubeNode): void;
  getSources?(): ReadonlyMap<string, HTMLElement>;
  onCardsChanged?(): void;
}

/** Retain shared Cube presentation while measuring only masonry geometry. */
export class WorkflowMasonryView {
  readonly element: HTMLElement;
  readonly backdrop: HTMLElement;
  readonly #content: HTMLElement;
  readonly #overrideSlot: HTMLElement;
  readonly #options: WorkflowMasonryViewOptions;
  readonly #views = new Map<string, WorkflowMasonrySectionView>();
  readonly #geometry: NativeCardGeometryObserver;
  readonly #remounts: WorkflowMasonryRemountQueue;
  readonly #motion = new SurfaceMotion();
  readonly #stackInset = new MasonryStackInsetMotion();
  readonly #presentation: SurfacePresentationBoundary;
  #sections: readonly WorkflowMasonrySection[];
  #generation = 0;
  #disposed = false;
  #mounting = true;
  #transitioning = false;
  #pendingLayout = false;
  #searchCards: ReadonlySet<string> | null = null;
  #previewCubeOrder: readonly string[] | null = null;

  /** Prepare a continuous surface whose sections share existing Cube presentation owners. */
  constructor(options: WorkflowMasonryViewOptions) {
    ensureCubeSurfaceStyles(options.document);
    this.#options = options;
    this.#presentation = new SurfacePresentationBoundary(options.document);
    this.#sections = options.sections;
    this.backdrop = options.document.createElement('div');
    this.backdrop.className = 'sugarcubes-workflow-masonry__backdrop';
    this.backdrop.style.opacity = '0';
    this.backdrop.setAttribute('aria-hidden', 'true');
    this.element = options.document.createElement('section');
    this.element.className = 'sugarcubes-workflow-masonry';
    this.element.style.opacity = '1';
    this.element.dataset.scMasonryPreparing = '';
    this.element.setAttribute('aria-label', 'Workflow masonry');
    // Comfy's ancestor capture handler checks this before forwarding wheel input to the graph.
    this.element.setAttribute('data-capture-wheel', 'true');
    this.element.tabIndex = 0;
    this.element.addEventListener('wheel', this.#onWheel, { passive: false });
    this.element.addEventListener(DISCLOSURE_LAYOUT_EVENT, this.#onDisclosureLayout);
    options.document.addEventListener('pointerdown', this.#onInteraction, true);
    options.document.addEventListener('keydown', this.#onInteraction, true);
    options.document.addEventListener(COMPACT_MASONRY_CHANGE, this.#onCompactChange);
    this.#content = options.document.createElement('div');
    this.#content.className = 'sugarcubes-workflow-masonry__content';
    this.#overrideSlot = options.document.createElement('div');
    this.#overrideSlot.className = 'sc-plugin-overrides-slot';
    this.element.append(this.#overrideSlot, this.#content);
    this.#geometry = new NativeCardGeometryObserver(options.document, () => {
      if (!this.#mounting) this.#requestLayout();
    });
    this.#remounts = new WorkflowMasonryRemountQueue(
      () => this.#options.getSources?.() ?? new Map(),
      () => {
        if (this.#mounting || this.element.hasAttribute('data-sc-masonry-preparing')) return;
        this.#geometry.request();
        this.#options.onCardsChanged?.();
      },
    );
  }

  /** Read original source geometry before attaching the new surface or changing graph interaction. */
  prepareSources(): ReadonlyMap<string, HTMLElement> {
    const sources = this.#options.getSources?.() ?? new Map<string, HTMLElement>();
    measureNativeCardPresentationSources(sources.values());
    return sources;
  }

  /** Mount native controls through the same path used for subsequent presentation changes. */
  mountCards(
    renderer: NativeNodeCardRenderer,
    sources: ReadonlyMap<string, HTMLElement> = this.prepareSources(),
  ): Promise<void> {
    prepareNativePresentation(this.element);
    return this.refresh(this.#sections, renderer, sources);
  }

  /** Retain section headers and open menus while reconciling card decisions in small batches. */
  async refresh(
    sections: readonly WorkflowMasonrySection[],
    renderer: NativeNodeCardRenderer,
    sources: ReadonlyMap<string, HTMLElement> = this.#options.getSources?.() ?? new Map(),
  ): Promise<void> {
    if (this.#disposed) return;
    const generation = ++this.#generation;
    this.#mounting = true;
    this.#sections = sections;
    this.#previewCubeOrder = null;
    const ids = new Set(sections.map((section) => section.id));
    for (const [id, view] of this.#views) {
      if (!ids.has(id)) {
        this.#motion.forget(view.header);
        view.dispose();
        this.#views.delete(id);
      }
    }
    const options = this.#layoutOptions();
    const pending: WorkflowMasonryPendingMount[] = [];
    let projectionOrder = 0;
    for (const section of sections) {
      let view = this.#views.get(section.id);
      if (!view) {
        view = new WorkflowMasonrySectionView(section, {
          content: this.#content,
          cardState: this.#options.cardState,
          openCube: this.#options.openCube,
          motion: this.#motion,
          requestRemount: (id, rebind) => this.#remounts.request(id, rebind),
        });
        this.#views.set(section.id, view);
      }
      pending.push(...view.prepare(section, renderer, sources));
      for (const card of section.cards) {
        const cell = view.cells.get(card.id);
        if (cell) cell.dataset.scProjectionOrder = String(projectionOrder++);
      }
    }
    this.#content.querySelector('.sugarcubes-workflow-masonry__empty')?.remove();
    if (sections.length === 0) {
      const empty = this.#content.ownerDocument.createElement('p');
      empty.className = 'sugarcubes-workflow-masonry__empty';
      empty.textContent = 'Add nodes to your workflow to see them here.';
      this.#content.append(empty);
    }
    this.#assignWidths(
      computeWorkflowMasonry(
        sections.map((section) => ({
          id: section.id,
          cards: section.cards
            .filter((card) => card.visible)
            .map((card) => ({
              id: card.id,
              columnSpan: card.columnSpan,
              height: 36,
            })),
        })),
        options,
      ),
    );
    let nativeMounts = 0;
    for (let index = 0; index < pending.length; index += 1) {
      if (this.#disposed || generation !== this.#generation) return;
      const item = pending[index];
      item?.mount();
      if (item?.requiresNativeMount) nativeMounts += 1;
      if (item?.requiresNativeMount && nativeMounts % 6 === 0 && index + 1 < pending.length) {
        await new Promise<void>((resolve) => {
          const windowRef = this.element.ownerDocument.defaultView;
          if (windowRef?.requestAnimationFrame) windowRef.requestAnimationFrame(() => resolve());
          else queueMicrotask(resolve);
        });
      }
    }
    if (this.#disposed || generation !== this.#generation) return;
    this.#mounting = false;
    this.#geometry.observe([
      this.element,
      this.#overrideSlot,
      ...[...this.#views.values()].flatMap((view) => [...view.cells.values()]),
    ]);
    this.#requestLayout();
  }

  /** Pack visible cards into section runs using one shared column width. */
  layout(): void {
    if (this.#disposed) return;
    if (this.#transitioning) {
      this.#pendingLayout = true;
      return;
    }
    const options = this.#layoutOptions();
    const measure = (): ReturnType<typeof computeWorkflowMasonry> =>
      computeWorkflowMasonry(
        this.#layoutSections().map((section) => ({
          id: section.id,
          cards: section.cards
            .filter((card) => card.visible && this.#searchVisible(card.id))
            .map((card) => ({
              id: card.id,
              columnSpan: card.columnSpan,
              height: this.#measureCard(section.id, card.id),
            })),
        })),
        options,
      );
    const cells = new Map([...this.#views.values()].flatMap((view) => [...view.cells]));
    const initial = measure();
    const widthChanged = this.#assignWidths(initial);
    const result = widthChanged ? measure() : initial;
    const placements: SurfacePlacement[] = [];
    for (const section of result.sections) {
      const header = this.#views.get(section.id)?.header;
      if (header) {
        header.dataset.masonryY = String(section.y);
        header.dataset.masonryHeight = String(section.height);
        header.style.setProperty('--sc-masonry-section-height', `${section.height}px`);
        placements.push({
          element: header,
          rectangle: {
            x: 0,
            y: section.y,
            width: options.availableWidth,
            height: 36,
          },
        });
      }
    }
    for (const card of result.placements) {
      const cell = cells.get(card.id);
      if (cell) placements.push({ element: cell, rectangle: card });
    }
    this.#motion.place(placements);
    this.#motion.resizeHeight(this.#content, Math.max(result.height, 120));
    this.element.dispatchEvent(new CustomEvent('sc-masonry-layout'));
  }

  /** Expose the lifecycle-owned projection without rediscovering graph candidates from presentation events. */
  get sections(): readonly WorkflowMasonrySection[] {
    return this.#sections;
  }

  /** Reserve rail space through ordinary layout so native controls animate rather than remount. */
  setStackWidth(width: number): void {
    if (!Number.isFinite(width) || width < 0)
      throw new TypeError('Cube stack width must be finite and nonnegative.');
    this.#stackInset.set(this.element, this.#content, width, () => this.#requestLayout());
  }

  /** Preview an exact section permutation through existing compositor motion without editing workflow state. */
  previewCubeOrder(sectionIds: readonly string[] | null): void {
    previewMasonrySections(this.#sections, sectionIds);
    this.#previewCubeOrder = sectionIds === null ? null : [...sectionIds];
    this.#requestLayout();
  }

  /** Accept canonical reading order while retaining every mounted section and native control. */
  setCubeOrder(sectionIds: readonly string[]): void {
    const current = this.#sections
      .filter((section) => section.cube !== null)
      .map((section) => section.id);
    if (
      current.every((id, index) => id === sectionIds[index]) &&
      current.length === sectionIds.length
    )
      return;
    this.#sections = [...previewMasonrySections(this.#sections, sectionIds)];
    this.#requestLayout();
  }

  /** Return identified mounted cells for scoped native-field adapters. */
  cards(): ReadonlyMap<string, HTMLElement> {
    return new Map([...this.#views.values()].flatMap((view) => [...view.cells]));
  }

  /** Pack finder-visible cards without changing persistent node visibility or native mount identity. */
  setSearchCards(cards: ReadonlySet<string> | null, remeasure = false): void {
    const same =
      cards === this.#searchCards ||
      (cards !== null &&
        this.#searchCards !== null &&
        cards.size === this.#searchCards.size &&
        [...cards].every((id) => this.#searchCards?.has(id)));
    if (same && !remeasure) return;
    this.#searchCards = cards;
    for (const view of this.#views.values()) {
      let visible = false;
      for (const [id, cell] of view.cells) {
        const shown = this.#searchVisible(id);
        cell.toggleAttribute('data-sc-search-card-hidden', !shown);
        visible ||= shown;
      }
      view.header.toggleAttribute('data-sc-search-card-hidden', cards !== null && !visible);
    }
    this.#requestLayout();
  }

  /** Keep empty filtered sections out of packing while retaining their actual mounted headers. */
  #layoutSections(): readonly WorkflowMasonrySection[] {
    const sections = previewMasonrySections(this.#sections, this.#previewCubeOrder);
    return this.#searchCards === null
      ? sections
      : sections.filter((section) =>
          section.cards.some((card) => card.visible && this.#searchVisible(card.id)),
        );
  }

  /** Resolve the transient finder mask independently of policy visibility. */
  #searchVisible(id: string): boolean {
    return this.#searchCards === null || this.#searchCards.has(id);
  }

  /** Read container constraints once before any card width writes. */
  #layoutOptions(): CubeMasonryOptions {
    const gap = Number.parseFloat(
      getComputedStyle(this.element).getPropertyValue('--sugarcubes-masonry-gap'),
    );
    return {
      availableWidth: Math.max(1, this.#content.clientWidth),
      minimumColumnWidth:
        Number.parseFloat(
          getComputedStyle(this.element).getPropertyValue('--sugarcubes-masonry-column-width'),
        ) || 320,
      gap: Number.isFinite(gap) ? Math.max(0, gap) : 12,
    };
  }

  /** Establish native control widths before mount so text and sliders measure their final columns. */
  #assignWidths(layout: ReturnType<typeof computeWorkflowMasonry>): boolean {
    const cells = new Map([...this.#views.values()].flatMap((view) => [...view.cells]));
    let changed = false;
    for (const card of layout.placements) {
      const cell = cells.get(card.id);
      const width = `${String(card.width)}px`;
      if (cell && cell.style.width !== width) {
        cell.style.width = width;
        changed = true;
      }
    }
    return changed;
  }

  /** Use settled disclosure geometry so neighbors share one smooth destination transition. */
  #measureCard(sectionId: string, cardId: string): number {
    const cell = this.#views.get(sectionId)?.cells.get(cardId);
    return cell ? measureDisclosureCardHeight(cell) : 36;
  }

  /** Pack simultaneous disclosure destinations together in the existing geometry frame. */
  readonly #onDisclosureLayout = (): void => {
    if (!this.#mounting) this.#geometry.request();
  };

  /** Coalesce initial native styling into the single endpoint layout that starts assembly. */
  #requestLayout(): void {
    if (this.element.hasAttribute('data-sc-masonry-preparing')) this.#pendingLayout = true;
    else this.layout();
  }

  /** Freeze scroll coordinates during card travel and keep wheel input inside the native surface. */
  readonly #onWheel = (event: WheelEvent): void => {
    if (this.#transitioning) event.preventDefault();
    event.stopPropagation();
  };

  /** Navigate by stable section identity for shared Cube stack navigation. */
  scrollToSection(id: string): void {
    const header = this.#views.get(id)?.header;
    if (header)
      this.element.scrollTo({
        top:
          header.offsetTop +
          Number(header.dataset.masonryY ?? 0) +
          (this.#overrideSlot.childElementCount
            ? this.#content.offsetTop - this.#overrideSlot.offsetTop
            : 0),
        behavior: this.#options.document.defaultView?.matchMedia?.(
          '(prefers-reduced-motion: reduce)',
        ).matches
          ? 'instant'
          : 'smooth',
      });
  }

  /** Expose complete painted cells after SurfaceMotion commits their settled placements. */
  transitionCards(): WorkflowMasonryTransitionCard[] {
    return [...this.#views.values()].flatMap((view) =>
      [...view.cells].flatMap(([id, cell]) => {
        return cell.querySelector('.sugarcubes-cube-face__native-card-mount') ? [{ id, cell }] : [];
      }),
    );
  }

  /** Wait for native mount or restored graph styles before measuring animation endpoints. */
  settlePresentation(): Promise<void> {
    if (this.element.hasAttribute('data-sc-masonry-preparing')) {
      prepareNativePresentation(this.element);
      return this.#presentation.settled(1);
    }
    return this.#stackInset.settled().then(() => {
      if (!this.#disposed) return this.#presentation.settled();
    });
  }

  /** Hold assembly endpoints and keep restored hydration instantaneous until the next user intent. */
  setTransitioning(transitioning: boolean, animateLayout = true): void {
    if (transitioning && !this.#transitioning) {
      this.element.scrollTo?.({ top: this.element.scrollTop, behavior: 'instant' });
      if (this.#pendingLayout || this.element.hasAttribute('data-sc-masonry-preparing')) {
        this.#pendingLayout = false;
        this.layout();
      }
      this.#motion.finishAnimations();
    }
    this.#transitioning = transitioning;
    if (!transitioning && animateLayout) this.#motion.settle();
    if (!transitioning && this.#pendingLayout) {
      this.#pendingLayout = false;
      this.layout();
    }
  }

  /** Enable subsequent responsive and disclosure motion after the restored view receives user intent. */
  readonly #onInteraction = (): void => {
    if (!this.#transitioning && !this.element.hasAttribute('data-sc-masonry-preparing'))
      this.#motion.settle();
  };

  /** Repack mounted cards when the shared density preference changes. */
  readonly #onCompactChange = (): void => this.#requestLayout();

  /** Release observers and native mounts before restoring singleton custom widgets. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#options.document.removeEventListener('pointerdown', this.#onInteraction, true);
    this.#options.document.removeEventListener('keydown', this.#onInteraction, true);
    this.#options.document.removeEventListener(COMPACT_MASONRY_CHANGE, this.#onCompactChange);
    this.element.removeEventListener(DISCLOSURE_LAYOUT_EVENT, this.#onDisclosureLayout);
    this.element.removeEventListener('wheel', this.#onWheel);
    this.#generation += 1;
    this.#remounts.dispose();
    this.#geometry.dispose();
    this.#presentation.cancel();
    this.#motion.dispose();
    this.#stackInset.dispose();
    restoreNativeCardPresentations(
      this.element.querySelectorAll<HTMLElement>('.sugarcubes-cube-face__node-card'),
    );
    for (const view of this.#views.values()) view.dispose();
    this.#views.clear();
    this.element.remove();
    this.backdrop.remove();
  }
}
