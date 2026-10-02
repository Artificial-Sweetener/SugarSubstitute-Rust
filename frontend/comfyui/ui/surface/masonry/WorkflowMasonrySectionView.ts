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
/** Compose existing Cube card presentation within one continuous workflow section. */
import type { CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import { ComfyCubeCardVisibilityToolboxPresenter } from '../../affordance/ComfyCubeCardVisibilityToolboxPresenter.js';
import { ComfyToolboxCheckMenuPresenter } from '../../affordance/ComfyToolboxCheckMenuPresenter.js';
import { ComfyToolboxTooltipPresenter } from '../../affordance/ComfyToolboxTooltipPresenter.js';
import type { CubeFaceCardStateService } from '../CubeFaceCardStateService.js';
import { NativeNodeCardHost } from '../NativeNodeCardHost.js';
import type { NativeNodeCardRenderer } from '../NativeNodeCardRenderer.js';
import { NativeWidgetElementLease } from './NativeWidgetElementLease.js';
import { NativeForwardedWidgetLease } from './NativeForwardedWidgetLease.js';
import { resolveMasonryWidgetSources } from './WorkflowMasonryWidgetSources.js';
import type { WorkflowMasonrySection } from './WorkflowMasonryProjection.js';

export interface WorkflowMasonrySectionViewOptions {
  content: HTMLElement;
  cardState: CubeFaceCardStateService;
  openCube(node: CubeNode): void;
}

/** Retain the shared optional-node menu while reconciling native card membership. */
export class WorkflowMasonrySectionView {
  readonly header: HTMLElement;
  readonly cells = new Map<string, HTMLElement>();
  readonly #options: WorkflowMasonrySectionViewOptions;
  readonly #title: HTMLElement;
  readonly #count: HTMLElement;
  readonly #anchor: HTMLElement;
  readonly #visibility: ComfyCubeCardVisibilityToolboxPresenter;
  readonly #forwarded: NativeForwardedWidgetLease[] = [];
  #host: NativeNodeCardHost | null = null;
  #widgets: NativeWidgetElementLease | null = null;
  #signature = '';
  #generation = 0;

  /** Build stable section chrome using the existing optional-node toolbox presenter. */
  constructor(section: WorkflowMasonrySection, options: WorkflowMasonrySectionViewOptions) {
    this.#options = options;
    const document = options.content.ownerDocument;
    this.header = document.createElement('header');
    this.header.className = 'sugarcubes-workflow-masonry__section';
    this.header.dataset.masonrySection = section.id;
    this.#title = document.createElement('h2');
    this.#count = document.createElement('small');
    const actions = document.createElement('div');
    actions.className = 'sugarcubes-workflow-masonry__actions';
    this.#anchor = document.createElement(section.cube ? 'button' : 'span');
    const cube = section.cube;
    if (cube) {
      this.#anchor.textContent = 'Open Cube';
      this.#anchor.setAttribute('aria-label', `Open ${section.label}`);
      this.#anchor.addEventListener('click', () => options.openCube(cube));
    }
    actions.append(this.#anchor);
    this.header.append(this.#title, this.#count, actions);
    options.content.append(this.header);
    this.#visibility = new ComfyCubeCardVisibilityToolboxPresenter({
      document,
      owner: options.cardState,
      tooltip: new ComfyToolboxTooltipPresenter(document),
      menu: new ComfyToolboxCheckMenuPresenter({
        document,
        featureMenuAttribute: 'data-sugarcubes-card-visibility-menu',
        featureItemAttribute: 'data-sugarcubes-card-visibility-row',
      }),
    });
  }

  /** Reconcile shared policy decisions without replacing the live menu or untouched controls. */
  prepare(section: WorkflowMasonrySection, renderer: NativeNodeCardRenderer): Array<() => void> {
    this.#title.textContent = section.label;
    this.#count.textContent = `${String(section.cards.filter((card) => card.visible).length)} nodes`;
    this.#visibility.present(this.#anchor, section.target);
    const signature = JSON.stringify(
      section.cards.map((card) => [
        card.id,
        card.node.type,
        card.label,
        card.visible,
        card.showActivationControl,
        card.columnSpan,
      ]),
    );
    if (signature === this.#signature) {
      this.#host?.refresh(section.cards);
      return [];
    }
    this.#signature = signature;
    this.#releaseCards();
    const generation = this.#generation;
    const host = new NativeNodeCardHost(renderer);
    this.#host = host;
    this.#widgets = new NativeWidgetElementLease([
      ...section.cards.filter((card) => card.visible).map((card) => card.node),
      ...(section.cube ? [section.cube] : []),
    ]);
    return section.cards
      .filter((card) => card.visible)
      .map((card) => {
        const cell = this.header.ownerDocument.createElement('article');
        cell.className = 'sugarcubes-workflow-masonry__card';
        cell.style.position = 'absolute';
        cell.dataset.masonryCard = card.id;
        this.#options.content.append(cell);
        this.cells.set(card.id, cell);
        return () => {
          if (generation !== this.#generation) return;
          const widgetSources = section.cube
            ? resolveMasonryWidgetSources(section.cube, card.node)
            : [];
          this.#forwarded.push(new NativeForwardedWidgetLease(cell, card.node, widgetSources));
          host.mountCard(
            cell,
            card,
            (node, enabled) =>
              this.#options.cardState.setEnabled(section.target, String(node.id ?? ''), enabled),
            { presentationGeometry: true, preserveNativeBody: true, widgetSources },
          );
        };
      });
  }

  /** Release all native leases and section chrome when its workflow membership ends. */
  dispose(): void {
    this.#releaseCards();
    this.#visibility.dispose();
    this.header.remove();
  }

  /** Restore singleton controls before preparing their next native mounts. */
  #releaseCards(): void {
    this.#generation += 1;
    for (const lease of this.#forwarded.splice(0)) lease.dispose();
    this.#host?.dispose();
    this.#host = null;
    this.#widgets?.restore();
    this.#widgets = null;
    for (const cell of this.cells.values()) cell.remove();
    this.cells.clear();
  }
}
