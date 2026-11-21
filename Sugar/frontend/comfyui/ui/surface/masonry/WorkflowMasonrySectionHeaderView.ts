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
/** Compose Cube identity and shared section actions on the masonry surface. */
import { requireCubeIdentity, type CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import { resolveCubeIdentityPresentation } from '../../cube/CubeIdentityPresentation.js';
import { ComfyCubeCardVisibilityToolboxPresenter } from '../../affordance/ComfyCubeCardVisibilityToolboxPresenter.js';
import { ComfyToolboxCheckMenuPresenter } from '../../affordance/ComfyToolboxCheckMenuPresenter.js';
import { ComfyToolboxTooltipPresenter } from '../../affordance/ComfyToolboxTooltipPresenter.js';
import type { CubeFaceCardStateService } from '../CubeFaceCardStateService.js';
import { createCubeModelTitleElement } from '../CubeModelPillDomRenderer.js';
import type { WorkflowMasonrySection } from './WorkflowMasonryProjection.js';
import { createButton } from '../appearance/ControlFactory.js';
import {
  readDocumentVisualStyle,
  subscribeVisualStyle,
} from '../appearance/VisualStyleBoundary.js';

export interface WorkflowMasonrySectionHeaderViewOptions {
  document: Document;
  cardState: CubeFaceCardStateService;
  openCube(node: CubeNode): void;
}

/** Retain action/menu state while refreshing the shared Cube identity presentation. */
export class WorkflowMasonrySectionHeaderView {
  readonly element: HTMLElement;
  readonly #title: HTMLElement;
  readonly #actions: HTMLElement;
  readonly #open: (() => void) | null;
  #anchor: HTMLElement;
  #section: WorkflowMasonrySection;
  readonly #visibility: ComfyCubeCardVisibilityToolboxPresenter;
  readonly #stopStyle: () => void;
  #identitySignature = '';

  /** Build stable section chrome without owning Cube identity or optional-node policies. */
  constructor(section: WorkflowMasonrySection, options: WorkflowMasonrySectionHeaderViewOptions) {
    const document = options.document;
    this.element = document.createElement('header');
    this.element.className = 'sugarcubes-workflow-masonry__section';
    this.element.dataset.masonrySection = section.id;
    this.#title = document.createElement('h2');
    this.#actions = document.createElement('div');
    this.#actions.className = 'sugarcubes-workflow-masonry__actions';
    this.#section = section;
    const cube = section.cube;
    this.#open = cube ? () => options.openCube(cube) : null;
    this.#anchor = this.#createAnchor();
    this.#actions.append(this.#anchor);
    this.element.append(this.#title, this.#actions);
    this.#visibility = new ComfyCubeCardVisibilityToolboxPresenter({
      document,
      owner: options.cardState,
      tooltip: new ComfyToolboxTooltipPresenter(document),
      menu: new ComfyToolboxCheckMenuPresenter({
        document,
        featureMenuAttribute: 'data-sugarcubes-card-visibility-menu',
        featureItemAttribute: 'data-sugarcubes-card-visibility-row',
      }),
      createButton: () => iconButton(document),
    });
    // Each visual style brings its own button; a change of style replaces both actions.
    this.#stopStyle = subscribeVisualStyle(document, () => {
      const previous = this.#anchor;
      this.#visibility.clear();
      this.#anchor = this.#createAnchor();
      previous.replaceWith(this.#anchor);
      this.#present(this.#section);
    });
  }

  /** Make the action that opens the Cube in the graph, or a plain anchor for a loose run. */
  #createAnchor(): HTMLElement {
    const document = this.element.ownerDocument;
    if (!this.#open) return document.createElement('span');
    const button = iconButton(document);
    const icon = document.createElement('i');
    icon.className = 'icon-[comfy--workflow] size-4';
    icon.setAttribute('aria-hidden', 'true');
    button.append(icon);
    const open = this.#open;
    button.addEventListener('click', () => open());
    return button;
  }

  /** Name the open action for its Cube and place the eye action beside it. */
  #present(section: WorkflowMasonrySection): void {
    if (section.cube) {
      this.#anchor.setAttribute('aria-label', `Open ${section.label}`);
      this.#anchor.title = `Open ${section.label} in graph`;
    }
    this.#visibility.present(this.#anchor, section.target);
  }

  /** Share graph-header icon and model-pill rules while leaving loose runs as plain titles. */
  render(section: WorkflowMasonrySection): void {
    const cube = section.cube;
    const identity = cube
      ? resolveCubeIdentityPresentation({
          metadata: requireCubeIdentity(cube),
          instanceTitle: section.label,
          fallbackDefinitionTitle: cube.subgraph.name,
        })
      : null;
    const signature = JSON.stringify([section.label, identity?.instanceModelTitle]);
    if (signature !== this.#identitySignature) {
      this.#identitySignature = signature;
      this.#title.setAttribute('aria-label', section.label);
      if (identity) {
        this.#title.replaceChildren(
          createCubeModelTitleElement(this.element.ownerDocument, identity.instanceModelTitle, {
            punchoutColor: 'var(--sugarcubes-masonry-surface, var(--comfy-menu-bg, #202020))',
          }),
        );
      } else this.#title.textContent = section.label;
    }
    this.#section = section;
    this.#present(section);
  }

  /** Release shared menu state and owned header DOM when the section ends. */
  dispose(): void {
    this.#stopStyle();
    this.#visibility.dispose();
    this.element.remove();
  }
}

/** Make one frameless icon-only action as the active visual style makes its buttons. */
function iconButton(document: Document): HTMLElement {
  // Comfy's own style has no button component to hand out; its button is a native one that
  // the header's styles face with Comfy's tokens.
  if (readDocumentVisualStyle(document) === 'comfy') {
    const native = document.createElement('button');
    native.type = 'button';
    native.dataset.scButton = '';
    return native;
  }
  // Header actions are frameless at rest and show their frame on hover, as the stack rail's
  // and the finder's icon buttons do: Fluent's subtle appearance, Spectrum's quiet button.
  const button = createButton(document);
  button.setAttribute('appearance', 'subtle');
  button.setAttribute('quiet', '');
  button.setAttribute('icon-only', '');
  return button;
}
