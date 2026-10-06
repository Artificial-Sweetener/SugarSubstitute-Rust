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
import { requireCubeIdentity, type CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import type { WorkflowMasonrySection } from './WorkflowMasonryProjection.js';
import type { NativeNodeCardRenderer } from '../NativeNodeCardRenderer.js';
import type { CubeFaceCardStateService } from '../CubeFaceCardStateService.js';
import { WorkflowMasonryView } from './WorkflowMasonryView.js';
import { WorkflowMasonryTransition } from './WorkflowMasonryTransition.js';

export interface WorkflowMasonryControllerOptions {
  document: Document;
  getContainer(): HTMLElement | null;
  isNodes2(): boolean;
  getSections(): WorkflowMasonrySection[];
  getRenderer(): Promise<NativeNodeCardRenderer>;
  getCardState(): CubeFaceCardStateService;
  openCube(node: CubeNode): void;
  notifyRendererFallback(): void;
  reportError(error: unknown): void;
  onStateChange(): void;
}

/** Keep layout choice transient and restore graph presentation on every failure path. */
export class WorkflowMasonryController {
  readonly #options: WorkflowMasonryControllerOptions;
  readonly #transition = new WorkflowMasonryTransition();
  #view: WorkflowMasonryView | null = null;
  #container: HTMLElement | null = null;
  #desired = false;
  #generation = 0;
  #mounting: Promise<void> | null = null;
  #topology = '';
  #renderer: NativeNodeCardRenderer | null = null;

  /** Bind host capabilities without importing host globals or owning graph mutations. */
  constructor(options: WorkflowMasonryControllerOptions) {
    this.#options = options;
  }

  /** Expose the requested presentation mode for button and future stack navigation. */
  get active(): boolean {
    return this.#desired;
  }

  /** Reverse the presentation with one authoritative state transition. */
  async toggle(): Promise<void> {
    if (!this.#options.isNodes2()) return;
    this.#desired = !this.#desired;
    const generation = ++this.#generation;
    this.#options.onStateChange();
    if (!this.#desired) {
      this.#setGraphHidden(false);
      await this.#animate(false);
      if (generation === this.#generation) this.#releaseView();
      return;
    }
    await this.#show(generation);
  }

  /** Release obsolete graph bindings immediately while retaining the chosen presentation. */
  async graphReplaced(): Promise<void> {
    if (!this.#desired) return;
    const generation = ++this.#generation;
    this.#transition.cancel();
    this.#releaseView();
    this.#options.onStateChange();
    await this.#show(generation);
  }

  /** Mount the current graph only while this presentation request still owns the lifecycle. */
  async #show(generation: number): Promise<void> {
    try {
      if (!this.#view) {
        const renderer = await this.#options.getRenderer();
        if (generation !== this.#generation || !this.#desired || !this.#options.isNodes2()) return;
        const container = this.#options.getContainer();
        if (!container) throw new Error('Comfy graph container is not mounted.');
        const sections = this.#options.getSections();
        this.#renderer = renderer;
        this.#topology = topologySignature(sections);
        this.#view = new WorkflowMasonryView({
          document: this.#options.document,
          sections,
          cardState: this.#options.getCardState(),
          openCube: (cube) => {
            this.leaveImmediately();
            this.#options.openCube(cube);
          },
        });
        this.#container = container;
        this.#view.element.style.opacity = '0';
        container.append(this.#view.element);
        this.#view.layout();
        this.#mounting = this.#view.mountCards(renderer);
      }
      await this.#mounting;
      if (generation !== this.#generation || !this.#desired) return;
      await this.#animate(true);
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
    const topology = topologySignature(sections);
    if (this.#topology === topology) return;
    this.#topology = topology;
    try {
      this.#mounting = this.#view.refresh(sections, this.#renderer);
      await this.#mounting;
    } catch (error: unknown) {
      if (generation !== this.#generation) return;
      this.leaveImmediately();
      this.#options.reportError(error);
    }
  }

  /** Fade the complete surface without reading or animating node geometry. */
  async #animate(entering: boolean): Promise<void> {
    const view = this.#view;
    if (!view) return;
    const windowRef = this.#options.document.defaultView;
    await this.#transition.run(
      view.element,
      entering,
      windowRef?.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false,
    );
  }

  /** Remove presentation resources while preserving canonical graph and edit state. */
  #releaseView(): void {
    this.#setGraphHidden(false);
    this.#view?.dispose();
    this.#view = null;
    this.#mounting = null;
    this.#renderer = null;
    this.#container = null;
  }

  /** Keep graph visible behind the fade and suppress it only after masonry is opaque. */
  #setGraphHidden(hidden: boolean): void {
    const container = this.#container;
    if (!container) return;
    const graphRoot = container.closest<HTMLElement>('#graph-canvas-container');
    for (const element of new Set([container, graphRoot])) {
      if (!element) continue;
      if (hidden) element.dataset.sugarcubesMasonry = 'true';
      else delete element.dataset.sugarcubesMasonry;
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
