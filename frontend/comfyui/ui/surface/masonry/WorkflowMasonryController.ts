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
/** Own the reversible workflow presentation state machine and renderer fallback. */
import { NativeRendererAcquisition } from '../NativeRendererAcquisition.js';
import { requireCubeIdentity, type CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import type { WorkflowMasonrySection } from './WorkflowMasonryProjection.js';
import type { NativeNodeCardRenderer } from '../NativeNodeCardRenderer.js';
import type { CubeFaceCardStateService } from '../CubeFaceCardStateService.js';
import { WorkflowMasonryView } from './WorkflowMasonryView.js';
import { WorkflowMasonryTransition } from './WorkflowMasonryTransition.js';
import type { WorkflowMasonryLinkMotion } from './WorkflowMasonryLinkMotion.js';
import { WorkflowMasonryGraphReveal } from './WorkflowMasonryGraphReveal.js';
import { SurfacePresentationBoundary } from '../motion/SurfacePresentationBoundary.js';
import {
  measureWorkflowMasonryGraph,
  workflowMasonryGraphLayers,
  workflowMasonryGraphCards,
} from './WorkflowMasonryGraphGeometry.js';

export interface WorkflowMasonryControllerOptions {
  document: Document;
  getContainer(): HTMLElement | null;
  getScope(): object | null;
  isNodes2(): boolean;
  getSections(): WorkflowMasonrySection[];
  getRenderer(): Promise<NativeNodeCardRenderer>;
  createLinkMotion?(container: HTMLElement): WorkflowMasonryLinkMotion | null;
  getCardState(): CubeFaceCardStateService;
  openCube(node: CubeNode): void;
  notifyRendererFallback(): void;
  reportError(error: unknown): void;
  onStateChange(): void;
  onSurfaceReady?(view: WorkflowMasonryView, preparing: boolean): void;
  onSurfaceLeaving?(): void;
  onSurfaceValuesChanged?(): void;
  onSurfaceTransition?(entering: boolean): Promise<void>;
}

/** Keep layout choice transient and restore graph presentation on every failure path. */
export class WorkflowMasonryController {
  readonly #options: WorkflowMasonryControllerOptions;
  readonly #transition = new WorkflowMasonryTransition();
  readonly #settled: SurfacePresentationBoundary;
  readonly #acquisition: NativeRendererAcquisition;
  #reveal: WorkflowMasonryGraphReveal | null = null;
  #view: WorkflowMasonryView | null = null;
  #container: HTMLElement | null = null;
  #desired = false;
  readonly #suspensions = new Set<'app' | 'configure' | 'subgraph'>();
  #generation = 0;
  #mounting: Promise<void> | null = null;
  #topology = '';
  #renderer: NativeNodeCardRenderer | null = null;
  #links: WorkflowMasonryLinkMotion | null = null;
  readonly #graphInert = new Map<HTMLElement, boolean>();
  readonly #hiddenGraphRoots = new Set<HTMLElement>();
  readonly #scrollPositions = new WeakMap<object, number>();
  #viewScope: object | null = null;

  /** Bind host capabilities without importing host globals or owning graph mutations. */
  constructor(options: WorkflowMasonryControllerOptions) {
    this.#options = options;
    this.#acquisition = new NativeRendererAcquisition(options.document);
    this.#settled = new SurfacePresentationBoundary(options.document);
  }

  /** Expose the requested presentation mode for button and future stack navigation. */
  get active(): boolean {
    return this.#desired && this.#suspensions.size === 0;
  }

  /** Retain graph-only control exclusion until the departing masonry surface is released. */
  get presenting(): boolean {
    return (
      (this.#desired && !this.#suspensions.has('app') && !this.#suspensions.has('subgraph')) ||
      this.#view !== null
    );
  }

  /** Restore a committed workflow choice without replaying an explicit user-toggle transition. */
  async restore(desired: boolean): Promise<void> {
    if (this.#desired === desired) return;
    this.#desired = desired;
    const generation = ++this.#generation;
    this.#transition.cancel();
    this.#releaseView(
      desired && !this.#suspensions.has('app') && !this.#suspensions.has('subgraph'),
    );
    this.#options.onStateChange();
    if (this.active && this.#options.isNodes2()) await this.#show(generation, false);
  }

  /** Release native mounts during host transitions without changing the user's view choice. */
  async setSuspended(reason: 'app' | 'configure' | 'subgraph', suspended: boolean): Promise<void> {
    if (this.#suspensions.has(reason) === suspended) return;
    if (suspended) this.#suspensions.add(reason);
    else this.#suspensions.delete(reason);
    const generation = ++this.#generation;
    this.#transition.cancel();
    this.#releaseView(
      this.#desired && !this.#suspensions.has('app') && !this.#suspensions.has('subgraph'),
    );
    this.#options.onStateChange();
    if (!this.active || !this.#options.isNodes2()) return;
    // Comfy commits its mode before Vue reveals the graph container and native controls.
    await this.#settled.settled(1);
    if (generation === this.#generation && this.active) await this.#show(generation, false);
  }

  /** Reverse the presentation with one authoritative state transition. */
  async toggle(): Promise<void> {
    if (!this.#options.isNodes2() || this.#suspensions.size) return;
    this.#desired = !this.#desired;
    const generation = ++this.#generation;
    this.#acquisition.cancel();
    this.#settled.cancel();
    this.#links?.hold(!this.#desired);
    this.#options.onStateChange();
    if (!this.#desired) {
      this.#setGraphHidden(false);
      try {
        if (!this.#view?.element.hasAttribute('data-sc-masonry-transition'))
          await this.#view?.settlePresentation();
        if (generation !== this.#generation) return;
        await this.#animate(false);
        if (generation !== this.#generation) return;
        const renderer = this.#renderer;
        this.#disposePresentation();
        renderer?.requestSlotLayoutSync?.();
        if (this.#links) {
          await this.#settled.settled();
          if (generation !== this.#generation || !this.#container) return;
          this.#links.dispose();
          this.#links = null;
          this.#links = this.#options.createLinkMotion?.(this.#container) ?? null;
          this.#links?.hold(true);
          await this.#links?.run(false, this.#reducedMotion());
        }
      } finally {
        if (generation === this.#generation) this.#releaseView();
      }
      return;
    }
    await this.#show(generation, true);
  }

  /** Mount the current graph only while this presentation request still owns the lifecycle. */
  async #show(generation: number, animate: boolean): Promise<void> {
    try {
      if (!this.#view) {
        const renderer = await this.#acquisition.acquire(() => this.#options.getRenderer());
        if (!renderer) return;
        if (generation !== this.#generation || !this.active || !this.#options.isNodes2()) return;
        const container = this.#options.getContainer();
        if (!container) throw new Error('Comfy graph container is not mounted.');
        const sections = this.#options.getSections();
        this.#renderer = renderer;
        this.#topology = topologySignature(sections);
        this.#view = new WorkflowMasonryView({
          document: this.#options.document,
          sections,
          cardState: this.#options.getCardState(),
          getSources: () => workflowMasonryGraphCards(container, this.#options.getSections()),
          openCube: (cube) => this.#options.openCube(cube),
          onCardsChanged: () => {
            if (this.#view && this.#desired) this.#options.onSurfaceReady?.(this.#view, animate);
          },
        });
        this.#container = container;
        this.#viewScope = this.#options.getScope();
        this.#reveal ??= new WorkflowMasonryGraphReveal();
        this.#reveal.reconcile(container, sections);
        const sources = this.#view.prepareSources();
        container.append(this.#view.backdrop, this.#view.element);
        this.#setGraphInert(true);
        this.#mounting = this.#view.mountCards(renderer, sources);
      }
      await this.#mounting;
      if (generation !== this.#generation || !this.#desired) return;
      if (this.#view?.element.hasAttribute('data-sc-masonry-preparing')) {
        this.#renderer?.requestSlotLayoutSync?.();
        await this.#view.settlePresentation();
      }
      if (generation !== this.#generation || !this.#desired) return;
      if (this.#view) {
        this.#options.onSurfaceReady?.(this.#view, animate);
        await this.#animate(true, animate);
        if (generation === this.#generation && this.#viewScope && this.#view)
          this.#view.element.scrollTop = this.#scrollPositions.get(this.#viewScope) ?? 0;
      }
      if (generation === this.#generation && this.#desired) this.#setGraphHidden(true);
    } catch (error: unknown) {
      if (generation !== this.#generation) return;
      this.leaveImmediately();
      this.#options.reportError(error);
    }
  }

  /** Restore graph immediately before graph replacement, renderer teardown, or application disposal. */
  leaveImmediately(): void {
    this.#desired = false;
    this.#generation += 1;
    this.#transition.cancel();
    this.#settled.cancel();
    this.#releaseView();
    this.#options.onStateChange();
  }

  /** Explain the unsupported renderer once, only when it interrupts active masonry. */
  rendererChanged(): void {
    const unsupported = !this.#options.isNodes2();
    if (unsupported && (this.#desired || this.#view)) {
      this.leaveImmediately();
      try {
        this.#options.notifyRendererFallback();
      } catch (error: unknown) {
        this.#options.reportError(error);
      }
    }
    this.#options.onStateChange();
  }

  /** Address sections by identity so a Cube stack can share this navigation surface. */
  scrollToSection(id: string): void {
    this.#view?.scrollToSection(id);
  }

  /** Reconcile card presentation without replaying transitions or closing the optional-node menu. */
  async graphChanged(): Promise<void> {
    if (!this.#desired || !this.#view || !this.#renderer) return;
    const generation = this.#generation;
    await this.#mounting;
    if (generation !== this.#generation || !this.#desired || !this.#view) return;
    const sections = this.#options.getSections();
    if (this.#container) this.#reveal?.reconcile(this.#container, sections);
    const topology = topologySignature(sections);
    if (this.#topology === topology) {
      this.#options.onSurfaceValuesChanged?.();
      return;
    }
    this.#topology = topology;
    try {
      this.#mounting = this.#view.refresh(sections, this.#renderer);
      await this.#mounting;
      if (generation === this.#generation && this.#desired && this.#view)
        this.#options.onSurfaceReady?.(this.#view, false);
    } catch (error: unknown) {
      if (generation !== this.#generation) return;
      this.leaveImmediately();
      this.#options.reportError(error);
    }
  }

  /** Assemble native cards from actual graph bounds without changing saved node positions. */
  async #animate(entering: boolean, animate = true): Promise<void> {
    const view = this.#view;
    const container = this.#container;
    if (!view || !container) return;
    this.#setGraphInert(true);
    view.setTransitioning(true);
    const reducedMotion = !animate || this.#reducedMotion();
    if (reducedMotion && this.#links) {
      const links = this.#links;
      this.#links = null;
      links.dispose();
    }
    if (!reducedMotion && !this.#links)
      this.#links = this.#options.createLinkMotion?.(container) ?? null;
    const noodles = entering ? this.#links?.run(true, reducedMotion) : undefined;
    await Promise.all([
      noodles,
      animate ? this.#options.onSurfaceTransition?.(entering) : undefined,
      this.#transition.run(
        {
          surface: view.element,
          backdrop: view.backdrop,
          cards: view.transitionCards(),
          graph: measureWorkflowMasonryGraph(container, this.#options.getSections()),
          ...(this.#reveal ? { reveal: this.#reveal } : {}),
        },
        entering,
        reducedMotion,
      ),
    ]);
    if (entering && !view.element.hasAttribute('data-sc-masonry-transition'))
      view.setTransitioning(false);
  }

  /** Remove presentation resources while preserving canonical graph and edit state. */
  #releaseView(retainGraphSuppression = false): void {
    this.#acquisition.cancel();
    if (!retainGraphSuppression) this.#setGraphHidden(false);
    this.#settled.cancel();
    const links = this.#links;
    this.#links = null;
    try {
      links?.dispose();
    } finally {
      this.#reveal?.dispose();
      this.#reveal = null;
      this.#disposePresentation();
      this.#container = null;
    }
  }

  /** Return borrowed controls before recapturing settled native noodle endpoints. */
  #disposePresentation(): void {
    if (this.#view && this.#viewScope)
      this.#scrollPositions.set(this.#viewScope, this.#view.element.scrollTop);
    this.#viewScope = null;
    // Rail reservation is part of the animation's coordinate space until card travel completes.
    if (this.#view) this.#options.onSurfaceLeaving?.();
    this.#view?.dispose();
    this.#view = null;
    this.#mounting = null;
    this.#renderer = null;
  }

  /** Respect the document's current motion preference on each transition phase. */
  #reducedMotion(): boolean {
    return (
      this.#options.document.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)')
        .matches ?? false
    );
  }

  /** Keep graph visible behind the fade and suppress it only after masonry is opaque. */
  #setGraphHidden(hidden: boolean): void {
    this.#setGraphInert(hidden);
    if (!hidden) {
      for (const element of this.#hiddenGraphRoots) delete element.dataset.sugarcubesMasonry;
      this.#hiddenGraphRoots.clear();
      return;
    }
    const container = this.#container;
    if (!container) return;
    const graphRoot = container.closest<HTMLElement>('#graph-canvas-container');
    for (const element of new Set([container, graphRoot])) {
      if (!element) continue;
      this.#hiddenGraphRoots.add(element);
      element.dataset.sugarcubesMasonry = 'true';
    }
  }

  /** Keep graph controls dormant while presentation owns the pointer, retaining host inert state. */
  #setGraphInert(hidden: boolean): void {
    if (!hidden) {
      for (const [element, inert] of this.#graphInert) element.inert = inert;
      this.#graphInert.clear();
    }
    const container = this.#container;
    if (!container) return;
    if (hidden) {
      for (const layer of workflowMasonryGraphLayers(container)) {
        if (!this.#graphInert.has(layer)) this.#graphInert.set(layer, layer.inert);
        if (!layer.inert) layer.inert = true;
      }
    }
  }
}

/** Fingerprint presentation membership without treating widget edits as remount requests. */
function topologySignature(sections: readonly WorkflowMasonrySection[]): string {
  return JSON.stringify(
    sections.map((section) => [
      section.id,
      section.label,
      section.cube ? requireCubeIdentity(section.cube) : null,
      section.cards.map((card) => [
        card.id,
        card.node.type,
        card.label,
        card.columnSpan,
        card.visible,
        card.enabled,
        card.showActivationControl,
      ]),
    ]),
  );
}
