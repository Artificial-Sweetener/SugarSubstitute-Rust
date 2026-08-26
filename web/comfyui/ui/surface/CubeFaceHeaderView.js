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
/** Compose the identity and action lanes of one Nodes 2.0 Cube header. */
import { createResolvedCubeIconElement } from '../core/CubeIconResolver.js';
import { CubeFaceActionsView } from './CubeFaceActionsView.js';
import { createCubeModelTitleElement } from './CubeModelPillDomRenderer.js';
import { createCubeUnsavedIndicator } from './CubeUnsavedIndicator.js';
/** Own the stable Cube header DOM independently from face layout and cards. */
export class CubeFaceHeaderView {
    element;
    #actions;
    /** Build all identity lanes and persistent action controls. */
    constructor(options) {
        this.element = options.document.createElement('header');
        this.element.className = 'sugarcubes-cube-face__header';
        this.#actions = new CubeFaceActionsView(options.document, options.metadata, options.chromeActions);
        if (options.identity.awaitingFirstSave) {
            this.#actions.element.prepend(createCubeUnsavedIndicator(options.document));
        }
        if (options.identity.isWild) {
            this.#actions.element.prepend(createWildCubeIndicator(options.document));
        }
        this.element.append(createInstanceIdentity(options.document, options.identity), createDefinitionIdentity(options.document, options.identity), this.#actions.element);
    }
    /** Reconcile header actions without rebuilding identity DOM. */
    renderActions() {
        this.#actions.render();
    }
    /** Release header-owned presentation state. */
    dispose() {
        this.element.remove();
    }
}
/** Build the graph-local instance title lane. */
function createInstanceIdentity(documentRef, identity) {
    const container = documentRef.createElement('div');
    container.className = 'sugarcubes-cube-face__identity';
    const icon = createResolvedCubeIconElement(documentRef, identity.icon, 'sugarcubes-cube-face__icon');
    const title = documentRef.createElement('strong');
    title.className = 'sugarcubes-cube-face__title';
    title.append(createCubeModelTitleElement(documentRef, identity.instanceModelTitle));
    container.append(icon, title);
    return container;
}
/** Build the centered definition/version and source lanes. */
function createDefinitionIdentity(documentRef, identity) {
    const badge = documentRef.createElement('div');
    badge.className = 'sugarcubes-cube-face__definition-badge';
    badge.dataset.cubeDefinitionBadge = '';
    const name = documentRef.createElement('span');
    name.className = 'sugarcubes-cube-face__definition-name';
    name.dataset.cubeDefinitionName = '';
    name.append(createCubeModelTitleElement(documentRef, identity.definitionModelTitle));
    const definitionSource = documentRef.createElement('span');
    definitionSource.className = 'sugarcubes-cube-face__definition-source';
    definitionSource.dataset.cubeDefinitionSource = '';
    const source = documentRef.createElement('span');
    source.className = 'sugarcubes-cube-face__source';
    source.textContent = identity.sourceLine;
    definitionSource.append(source);
    badge.append(name, definitionSource);
    return badge;
}
/** Mark an embedded definition without a matching library owner. */
function createWildCubeIndicator(documentRef) {
    const indicator = documentRef.createElement('span');
    indicator.className =
        'sugarcubes-cube-wild-indicator sugarcubes-cube-unsaved-indicator text-orange-400';
    indicator.title = 'Wild Cube';
    indicator.setAttribute('role', 'img');
    indicator.setAttribute('aria-label', 'Wild Cube');
    const icon = documentRef.createElement('i');
    icon.className = 'icon-[lucide--paw-print] size-4';
    icon.setAttribute('aria-hidden', 'true');
    indicator.append(icon);
    return indicator;
}
