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
/** Compose one Cube face around host-rendered native node cards. */

import type { ComfyNode } from '../types/graph.js';
import { CubeFaceActionsView } from './CubeFaceActionsView.js';
import {
  resolveCubeFaceCardPresentation,
  type CubeFaceCardDecision,
} from './CubeFaceCardPolicy.js';
import { setCubeFaceCardRevealed, setCubeFaceNodeEnabled } from './CubeFaceCardStateController.js';
import { orderCubeSurfaceCards } from './CubeMasonryLayout.js';
import { NativeNodeCardHost } from './NativeNodeCardHost.js';
import type { NativeNodeCardRenderer } from './NativeNodeCardRenderer.js';
import type { CubePreviewSnapshot } from './CubePreviewModel.js';
import { CubePreviewRailView } from './CubePreviewRailView.js';
import { layoutCubeSurfaceDom } from './CubeSurfaceDomLayout.js';
import type { CubeSurfaceState } from './CubeSurfaceState.js';
import { NativeCardGeometryObserver } from './NativeCardGeometryObserver.js';
import type { CubeIdentityPresentation } from '../cube/CubeIdentityPresentation.js';
import { createResolvedCubeIconElement } from '../core/CubeIconResolver.js';
import type { CubeFaceChromeActions, CubeFaceChromeMetadata } from './CubeFaceChromeActions.js';

export interface CubeSurfaceViewOptions {
  document: Document;
  renderer: NativeNodeCardRenderer;
  identity: CubeIdentityPresentation;
  metadata?: CubeFaceChromeMetadata;
  chromeActions?: CubeFaceChromeActions | null;
  nodes: readonly ComfyNode[];
  state: CubeSurfaceState;
  onStateChange(state: CubeSurfaceState): void;
  onMinimumHeightChange?(minimumHeight: number): void;
}

/** Own DOM composition, actions, and responsive geometry for one Cube face. */
export class CubeSurfaceView {
  readonly element: HTMLDivElement;
  readonly header: HTMLElement;
  readonly #state: CubeSurfaceState;
  readonly #nodes: ComfyNode[];
  readonly #onStateChange: (state: CubeSurfaceState) => void;
  readonly #onMinimumHeightChange: ((minimumHeight: number) => void) | null;
  readonly #cardHost: NativeNodeCardHost;
  readonly #actions: CubeFaceActionsView;
  readonly #content: HTMLDivElement;
  readonly #masonry: HTMLDivElement;
  readonly #previewRail: HTMLElement;
  readonly #previewView: CubePreviewRailView;
  readonly #geometryObserver: NativeCardGeometryObserver;
  #cells: HTMLElement[] = [];
  #visibleCards: CubeFaceCardDecision[] = [];
  #lastLayoutWidth = 1;

  /** Build a Cube view without assuming ownership of Comfy's renderer service. */
  constructor(options: CubeSurfaceViewOptions) {
    this.#state = options.state;
    this.#nodes = orderNodes(options.nodes, options.state.nodeOrder);
    this.#state.nodeOrder = this.#nodes.map((node) => String(node.id ?? ''));
    this.#onStateChange = options.onStateChange;
    this.#onMinimumHeightChange = options.onMinimumHeightChange ?? null;
    this.#cardHost = new NativeNodeCardHost(options.renderer);

    this.element = options.document.createElement('div');
    this.element.className = 'sugarcubes-cube-face';
    this.element.setAttribute('aria-label', `${options.identity.instanceTitle} Cube contents`);

    this.header = options.document.createElement('header');
    this.header.className = 'sugarcubes-cube-face__header';
    const identity = options.document.createElement('div');
    identity.className = 'sugarcubes-cube-face__identity';
    const icon = createResolvedCubeIconElement(
      options.document,
      options.identity.icon,
      'sugarcubes-cube-face__icon',
    );
    const title = options.document.createElement('strong');
    title.className = 'sugarcubes-cube-face__title';
    title.textContent = options.identity.instanceTitle;
    identity.append(icon, title);
    const definitionBadge = options.document.createElement('div');
    definitionBadge.className = 'sugarcubes-cube-face__definition-badge';
    definitionBadge.dataset.cubeDefinitionBadge = '';
    const definitionName = options.document.createElement('span');
    definitionName.className = 'sugarcubes-cube-face__definition-name';
    definitionName.dataset.cubeDefinitionName = '';
    definitionName.textContent = options.identity.definitionLine;
    const definitionSource = options.document.createElement('span');
    definitionSource.className = 'sugarcubes-cube-face__definition-source';
    definitionSource.dataset.cubeDefinitionSource = '';
    definitionSource.textContent = options.identity.sourceLine;
    definitionBadge.append(definitionName, definitionSource);
    this.#actions = new CubeFaceActionsView(
      options.document,
      options.metadata ?? {},
      options.chromeActions,
    );
    this.header.append(identity, definitionBadge, this.#actions.element);

    this.#content = options.document.createElement('div');
    this.#content.className = 'sugarcubes-cube-face__content';
    this.#content.dataset.cubeContent = '';

    this.#masonry = options.document.createElement('div');
    this.#masonry.className = 'sugarcubes-cube-face__masonry';
    this.#masonry.dataset.cubeMasonry = '';
    this.#masonry.style.position = 'relative';

    this.#previewRail = options.document.createElement('aside');
    this.#previewRail.className = 'sugarcubes-cube-face__preview';
    this.#previewRail.dataset.cubePreviewRail = '';
    this.#previewRail.setAttribute('aria-label', 'Cube output preview');
    this.#previewView = new CubePreviewRailView(this.#previewRail);

    this.#content.append(this.#masonry, this.#previewRail);
    this.element.append(this.header, this.#content);
    this.#geometryObserver = new NativeCardGeometryObserver(options.document, () =>
      this.layout(this.#lastLayoutWidth),
    );
    this.#renderCards();
  }

  /** Render current output and internal-node media into the dedicated rail. */
  renderPreview(snapshot: CubePreviewSnapshot): void {
    this.#previewView.render(snapshot);
  }

  /** Reflow masonry columns and the preview rail for the Cube's current width. */
  layout(width: number): void {
    const safeWidth = Number.isFinite(width) ? Math.max(1, width) : 1;
    this.#lastLayoutWidth = safeWidth;
    const result = layoutCubeSurfaceDom({
      width: safeWidth,
      state: this.#state,
      cards: this.#visibleCards,
      cells: this.#cells,
      content: this.#content,
      masonry: this.#masonry,
      previewRail: this.#previewRail,
    });
    if (result.minimumHeight !== null) {
      this.#onMinimumHeightChange?.(result.minimumHeight);
    }
  }

  /** Remove native card mounts owned by this view. */
  dispose(): void {
    this.#actions.dispose();
    this.#geometryObserver.dispose();
    this.#cardHost.dispose();
  }

  /** Reconcile exact native card mounts from the shared presentation policy. */
  #renderCards(): void {
    const presentation = resolveCubeFaceCardPresentation(this.#nodes, this.#state);
    this.#visibleCards = presentation.cards.filter((card) => card.visible);
    this.#cells = this.#cardHost.mount(this.#masonry, this.#visibleCards, (node, enabled) => {
      setCubeFaceNodeEnabled(this.#state, node, enabled);
      this.#commitCardChange();
    });
    this.#geometryObserver.observe(this.#cells);
    this.#actions.render(presentation.menuEntries, (nodeId, revealed) => {
      const node = this.#nodes.find((candidate) => String(candidate.id ?? '') === nodeId);
      if (!node) return;
      setCubeFaceCardRevealed(this.#state, node, revealed);
      this.#commitCardChange();
    });
  }

  /** Persist one card change, then remount against the resulting policy. */
  #commitCardChange(): void {
    this.#onStateChange(this.#state);
    this.#renderCards();
    this.layout(this.#lastLayoutWidth);
  }
}

/** Order real internal nodes by persisted Cube surface order. */
function orderNodes(nodes: readonly ComfyNode[], persistedOrder: readonly string[]): ComfyNode[] {
  const byId = new Map(nodes.map((node, index) => [String(node.id ?? index), node]));
  return orderCubeSurfaceCards(persistedOrder, [...byId.keys()])
    .map((id) => byId.get(id))
    .filter((node): node is ComfyNode => node !== undefined);
}
