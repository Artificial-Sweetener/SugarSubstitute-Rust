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
/** Render Cube output previews in one rail. */

import type { CubePreviewItem, CubePreviewSnapshot } from './CubePreviewModel.js';
import type { CubePreviewActions } from './CubePreviewActions.js';
import { resolveCubeOutputSections } from './CubePreviewSections.js';

/** Own safe preview media DOM for one Cube face. */
export class CubePreviewRailView {
  readonly element: HTMLElement;
  readonly #actions: CubePreviewActions | null;
  #signature = '';

  /** Bind one dedicated rail without owning preview collection. */
  constructor(element: HTMLElement, actions: CubePreviewActions | null = null) {
    this.element = element;
    this.#actions = actions;
  }

  /** Render one immutable preview snapshot without output-selection state. */
  render(snapshot: CubePreviewSnapshot): void {
    const signature = JSON.stringify(snapshot);
    if (signature === this.#signature) return;
    this.#signature = signature;

    const documentRef = this.element.ownerDocument;
    const media = documentRef.createElement('div');
    media.className = 'sugarcubes-cube-face__preview-media';
    const outputs = resolveCubeOutputSections(snapshot);
    const outputGrid = documentRef.createElement('div');
    outputGrid.className = 'sugarcubes-cube-face__preview-outputs';
    outputGrid.dataset.cubePreviewOutputs = '';
    outputGrid.style.gridTemplateRows = `repeat(${outputs.length}, minmax(0, 1fr))`;
    for (const output of outputs) {
      const section = documentRef.createElement('section');
      section.className = 'sugarcubes-cube-face__preview-output';
      section.dataset.cubePreviewOutput = output.id;
      section.setAttribute('aria-label', `Preview for ${output.label}`);
      const title = documentRef.createElement('header');
      title.className = 'sugarcubes-cube-face__preview-output-title';
      title.dataset.cubePreviewOutputTitle = '';
      const label = documentRef.createElement('span');
      label.dataset.cubePreviewOutputLabel = '';
      label.textContent = output.id;
      title.append(label);
      section.append(title);
      for (const item of output.items) {
        section.append(buildPreviewFigure(documentRef, item, this.#actions));
      }
      if (output.items.length === 0) {
        const empty = documentRef.createElement('p');
        empty.textContent = 'No preview available';
        section.append(empty);
      }
      outputGrid.append(section);
    }

    if (outputs.length === 0) {
      const empty = documentRef.createElement('p');
      empty.textContent = 'No preview available';
      media.append(empty);
    } else {
      media.append(outputGrid);
    }
    this.element.replaceChildren(media);
  }
}

/** Build one safe media figure without parsing dynamic host values as markup. */
function buildPreviewFigure(
  documentRef: Document,
  item: CubePreviewItem,
  actions: CubePreviewActions | null,
): HTMLElement {
  const figure = documentRef.createElement('figure');
  figure.dataset.cubePreviewItem = item.key;
  figure.setAttribute('aria-label', item.label);
  const image = documentRef.createElement('img');
  image.src = item.url;
  image.alt = '';
  image.loading = 'eager';
  figure.append(image);
  if (actions) {
    figure.addEventListener('contextmenu', (event) => actions.openContextMenu(item, event));
    const download = documentRef.createElement('button');
    download.type = 'button';
    download.className = 'sugarcubes-cube-face__preview-download';
    download.dataset.cubePreviewDownload = item.key;
    download.setAttribute('aria-label', 'Download image');
    download.title = 'Download image';
    const icon = documentRef.createElement('i');
    icon.classList.add('pi', 'pi-download');
    icon.setAttribute('aria-hidden', 'true');
    download.append(icon);
    download.addEventListener('pointerdown', (event) => event.stopPropagation());
    download.addEventListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();
      actions.download(item);
    });
    figure.append(download);
  }
  return figure;
}
