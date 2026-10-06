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
import type { CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import type { CubeFaceCardStateService } from '../CubeFaceCardStateService.js';
import type { NativeNodeCardRenderer } from '../NativeNodeCardRenderer.js';
import { NativeCardGeometryObserver } from '../NativeCardGeometryObserver.js';
import { ensureCubeSurfaceStyles } from '../CubeSurfaceStyles.js';
import { computeWorkflowMasonry } from './WorkflowMasonryLayout.js';
import type { WorkflowMasonrySection } from './WorkflowMasonryProjection.js';
import { WorkflowMasonrySectionView } from './WorkflowMasonrySectionView.js';
import { SurfaceMotion } from '../motion/SurfaceMotion.js';
import {
  DISCLOSURE_LAYOUT_EVENT,
  measureDisclosureCardHeight,
} from '../motion/DisclosureLayout.js';

export interface WorkflowMasonryViewOptions {
  document: Document;
  sections: readonly WorkflowMasonrySection[];
  cardState: CubeFaceCardStateService;
  openCube(node: CubeNode): void;
}

/** Retain shared Cube presentation while measuring only masonry geometry. */
export class WorkflowMasonryView {
  readonly element: HTMLElement;
  readonly #content: HTMLElement;
  readonly #options: WorkflowMasonryViewOptions;
  readonly #views = new Map<string, WorkflowMasonrySectionView>();
  readonly #geometry: NativeCardGeometryObserver;
  readonly #motion = new SurfaceMotion();
  #sections: readonly WorkflowMasonrySection[];
  #generation = 0;
  #disposed = false;
  #mounting = true;

  /** Prepare a continuous surface whose sections share existing Cube presentation owners. */
  constructor(options: WorkflowMasonryViewOptions) {
    ensureCubeSurfaceStyles(options.document);
    this.#options = options;
    this.#sections = options.sections;
    this.element = options.document.createElement('section');
    this.element.className = 'sugarcubes-workflow-masonry';
    this.element.setAttribute('aria-label', 'Workflow masonry');
    // Comfy's ancestor capture handler checks this before forwarding wheel input to the graph.
    this.element.setAttribute('data-capture-wheel', 'true');
    this.element.tabIndex = 0;
    this.element.addEventListener('wheel', (event) => event.stopPropagation(), { passive: true });
    this.element.addEventListener(DISCLOSURE_LAYOUT_EVENT, this.#onDisclosureLayout);
    this.#content = options.document.createElement('div');
    this.#content.className = 'sugarcubes-workflow-masonry__content';
    this.element.append(this.#content);
    this.#geometry = new NativeCardGeometryObserver(options.document, () => {
      if (!this.#mounting) this.layout();
    });
  }

  /** Mount native controls through the same path used for subsequent presentation changes. */
  mountCards(renderer: NativeNodeCardRenderer): Promise<void> {
    return this.refresh(this.#sections, renderer);
  }

  /** Retain section headers and open menus while reconciling card decisions in small batches. */
  async refresh(
    sections: readonly WorkflowMasonrySection[],
    renderer: NativeNodeCardRenderer,
  ): Promise<void> {
    if (this.#disposed) return;
    const generation = ++this.#generation;
    this.#mounting = true;
    this.#sections = sections;
    const ids = new Set(sections.map((section) => section.id));
    for (const [id, view] of this.#views) {
      if (!ids.has(id)) {
        this.#motion.forget(view.header);
        view.dispose();
        this.#views.delete(id);
      }
    }
    const pending: Array<() => void> = [];
    let projectionOrder = 0;
    for (const section of sections) {
      let view = this.#views.get(section.id);
      if (!view) {
        view = new WorkflowMasonrySectionView(section, {
          content: this.#content,
          cardState: this.#options.cardState,
          openCube: this.#options.openCube,
          motion: this.#motion,
        });
        this.#views.set(section.id, view);
      }
      pending.push(...view.prepare(section, renderer));
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
    for (let index = 0; index < pending.length; index += 1) {
      if (this.#disposed || generation !== this.#generation) return;
      pending[index]?.();
      if ((index + 1) % 6 === 0 && index + 1 < pending.length) {
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
      ...[...this.#views.values()].flatMap((view) => [...view.cells.values()]),
    ]);
    this.layout();
  }

  /** Pack visible cards into section runs using one shared column width. */
  layout(): void {
    if (this.#disposed) return;
    const options = {
      availableWidth: Math.max(1, this.#content.clientWidth),
      minimumColumnWidth:
        Number.parseFloat(
          getComputedStyle(this.element).getPropertyValue('--sugarcubes-masonry-column-width'),
        ) || 320,
      gap:
        Number.parseFloat(
          getComputedStyle(this.element).getPropertyValue('--sugarcubes-masonry-gap'),
        ) || 12,
    };
    const measure = (): ReturnType<typeof computeWorkflowMasonry> =>
      computeWorkflowMasonry(
        this.#sections.map((section) => ({
          id: section.id,
          cards: section.cards
            .filter((card) => card.visible)
            .map((card) => ({
              id: card.id,
              columnSpan: card.columnSpan,
              height: this.#measureCard(section.id, card.id),
            })),
        })),
        options,
      );
    const cells = new Map([...this.#views.values()].flatMap((view) => [...view.cells]));
    for (const card of measure().placements) {
      const cell = cells.get(card.id);
      if (cell && cell.style.width !== `${String(card.width)}px`)
        cell.style.width = `${String(card.width)}px`;
    }
    const result = measure();
    for (const section of result.sections) {
      const header = this.#views.get(section.id)?.header;
      if (header) {
        header.dataset.masonryY = String(section.y);
        this.#motion.place(header, {
          x: 0,
          y: section.y,
          width: options.availableWidth,
          height: 36,
        });
      }
    }
    for (const card of result.placements) {
      const cell = cells.get(card.id);
      if (cell) this.#motion.place(cell, card);
    }
    this.#motion.settle();
    this.#motion.resizeHeight(this.#content, Math.max(result.height, 120));
  }

  /** Use settled disclosure geometry so neighbors share one smooth destination transition. */
  #measureCard(sectionId: string, cardId: string): number {
    const cell = this.#views.get(sectionId)?.cells.get(cardId);
    return cell ? measureDisclosureCardHeight(cell) : 36;
  }

  /** Reflow at disclosure start and release, retaining native resize observation for other changes. */
  readonly #onDisclosureLayout = (): void => {
    if (!this.#mounting) this.layout();
  };

  /** Navigate by stable section identity for shared Cube stack navigation. */
  scrollToSection(id: string): void {
    const header = this.#views.get(id)?.header;
    if (header)
      this.element.scrollTo({
        top: header.offsetTop + Number(header.dataset.masonryY ?? 0),
        behavior: 'smooth',
      });
  }

  /** Release observers and native mounts before restoring singleton custom widgets. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.element.removeEventListener(DISCLOSURE_LAYOUT_EVENT, this.#onDisclosureLayout);
    this.#generation += 1;
    this.#geometry.dispose();
    this.#motion.dispose();
    for (const view of this.#views.values()) view.dispose();
    this.#views.clear();
    this.element.remove();
  }
}
