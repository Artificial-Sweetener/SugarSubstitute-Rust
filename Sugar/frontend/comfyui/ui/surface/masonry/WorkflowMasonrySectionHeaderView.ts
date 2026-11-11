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
import { createResolvedCubeIconElement } from '../../core/CubeIconResolver.js';
import { ComfyCubeCardVisibilityToolboxPresenter } from '../../affordance/ComfyCubeCardVisibilityToolboxPresenter.js';
import { ComfyToolboxCheckMenuPresenter } from '../../affordance/ComfyToolboxCheckMenuPresenter.js';
import { ComfyToolboxTooltipPresenter } from '../../affordance/ComfyToolboxTooltipPresenter.js';
import type { CubeFaceCardStateService } from '../CubeFaceCardStateService.js';
import { createCubeModelTitleElement } from '../CubeModelPillDomRenderer.js';
import type { WorkflowMasonrySection } from './WorkflowMasonryProjection.js';

export interface WorkflowMasonrySectionHeaderViewOptions {
  document: Document;
  cardState: CubeFaceCardStateService;
  openCube(node: CubeNode): void;
}

/** Retain action/menu state while refreshing the shared Cube identity presentation. */
export class WorkflowMasonrySectionHeaderView {
  readonly element: HTMLElement;
  readonly #title: HTMLElement;
  readonly #anchor: HTMLElement;
  readonly #visibility: ComfyCubeCardVisibilityToolboxPresenter;
  #identitySignature = '';

  /** Build stable section chrome without owning Cube identity or optional-node policies. */
  constructor(section: WorkflowMasonrySection, options: WorkflowMasonrySectionHeaderViewOptions) {
    const document = options.document;
    this.element = document.createElement('header');
    this.element.className = 'sugarcubes-workflow-masonry__section';
    this.element.dataset.masonrySection = section.id;
    this.#title = document.createElement('h2');
    const actions = document.createElement('div');
    actions.className = 'sugarcubes-workflow-masonry__actions';
    const cube = section.cube;
    this.#anchor = document.createElement(cube ? 'button' : 'span');
    if (cube) {
      this.#anchor.setAttribute('type', 'button');
      this.#anchor.className = 'sugarcubes-workflow-masonry__icon-action';
      const icon = document.createElement('i');
      icon.className = 'icon-[comfy--workflow] size-4';
      icon.setAttribute('aria-hidden', 'true');
      this.#anchor.append(icon);
      this.#anchor.addEventListener('click', () => options.openCube(cube));
    }
    actions.append(this.#anchor);
    this.element.append(this.#title, actions);
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
    const signature = JSON.stringify([section.label, identity?.icon, identity?.instanceModelTitle]);
    if (signature !== this.#identitySignature) {
      this.#identitySignature = signature;
      this.#title.setAttribute('aria-label', section.label);
      if (identity) {
        this.#title.replaceChildren(
          createResolvedCubeIconElement(
            this.element.ownerDocument,
            identity.icon,
            'sugarcubes-workflow-masonry__cube-icon',
          ),
          createCubeModelTitleElement(this.element.ownerDocument, identity.instanceModelTitle, {
            punchoutColor: 'var(--sugarcubes-masonry-surface, var(--comfy-menu-bg, #202020))',
          }),
        );
      } else this.#title.textContent = section.label;
    }
    if (cube) {
      this.#anchor.setAttribute('aria-label', `Open ${section.label}`);
      this.#anchor.title = `Open ${section.label} in graph`;
    }
    this.#visibility.present(this.#anchor, section.target);
  }

  /** Release shared menu state and owned header DOM when the section ends. */
  dispose(): void {
    this.#visibility.dispose();
    this.element.remove();
  }
}
