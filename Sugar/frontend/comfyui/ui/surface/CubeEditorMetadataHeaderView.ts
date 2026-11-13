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
/** Render the metadata card's shared model title and compact editing actions. */
import type { CubeModelTitlePresentation } from '../cube/CubeModelTitlePresentation.js';
import { presentCubeModelTitle } from './CubeModelPillDomRenderer.js';
import { createCubeUnsavedIndicator } from './CubeUnsavedIndicator.js';
import { createComfyPrimeIconElement } from './ComfyPrimeIcons.js';

export interface CubeEditorMetadataHeaderOptions {
  readonly title: CubeModelTitlePresentation;
  readonly unsaved: boolean;
  readonly editable: boolean;
  readonly saveEnabled: boolean;
  readonly collapsed: boolean;
  save(): void;
  toggle(): void;
}

/** Keep header presentation separate from metadata draft and save lifecycles. */
export class CubeEditorMetadataHeaderView {
  readonly element: HTMLElement;
  readonly #title: HTMLElement;
  readonly #save: HTMLButtonElement | null;

  /** Compose literal Cube identity, first-save feedback, and the available editing actions. */
  constructor(document: Document, options: CubeEditorMetadataHeaderOptions) {
    this.element = document.createElement('header');
    this.element.className = 'sugarcubes-cube-editor-metadata__titlebar';
    this.#title = document.createElement('strong');
    presentCubeModelTitle(this.#title, options.title);
    this.element.append(this.#title);
    if (options.unsaved) this.element.append(createCubeUnsavedIndicator(document));
    this.#save = options.editable ? this.#createSave(document, options) : null;
    if (this.#save) this.element.append(this.#save);
    this.element.append(this.#createCollapse(document, options));
  }

  /** Refresh edited identity and save availability without replacing active body controls. */
  edited(title: CubeModelTitlePresentation): void {
    presentCubeModelTitle(this.#title, title);
    if (this.#save) this.#save.disabled = false;
  }

  /** Disable repeated submissions until the draft owner publishes the next save state. */
  #createSave(document: Document, options: CubeEditorMetadataHeaderOptions): HTMLButtonElement {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'p-button p-component sugarcubes-cube-editor-metadata__save';
    button.setAttribute('aria-label', 'Save');
    button.title = 'Save Cube';
    button.append(createComfyPrimeIconElement(document, 'save'));
    button.disabled = !options.saveEnabled;
    button.addEventListener('click', () => {
      button.disabled = true;
      options.save();
    });
    return button;
  }

  /** Expose the draft owner's explicit roll-up action in either disclosure state. */
  #createCollapse(document: Document, options: CubeEditorMetadataHeaderOptions): HTMLButtonElement {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'p-button p-component sugarcubes-cube-editor-metadata__collapse';
    const label = options.collapsed ? 'Expand' : 'Minimize';
    button.setAttribute('aria-label', label);
    button.title = label;
    button.append(
      createComfyPrimeIconElement(document, options.collapsed ? 'chevron-down' : 'chevron-up'),
    );
    button.setAttribute('aria-expanded', String(!options.collapsed));
    button.addEventListener('click', options.toggle);
    return button;
  }
}
