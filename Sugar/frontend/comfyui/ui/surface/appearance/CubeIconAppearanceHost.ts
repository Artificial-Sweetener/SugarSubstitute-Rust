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
/** Project accepted palette polarity onto shared Cube icon variants without observing the document. */
import type { AppearanceSource } from './AppearanceState.js';

/** Own contrast selection for DOM icons across graph, masonry, stack, and dialogs. */
export class CubeIconAppearanceHost {
  readonly #style: HTMLStyleElement;
  readonly #unsubscribe: () => void;

  /** Follow the existing appearance authority while retaining every resolved icon element. */
  constructor(document: Document, source: AppearanceSource) {
    this.#style = document.createElement('style');
    this.#style.id = 'sugarcubes-cube-icon-appearance';
    document.head.append(this.#style);
    const refresh = (): void => {
      const tone = source.appearance.dark ? 'dark' : 'bright';
      this.#style.textContent = `
        [data-icon-tone] > .sugarcubes-cube-icon__contrast { display:none; width:100%; height:100%; object-fit:contain; }
        [data-icon-tone="${tone}"][data-icon-tone] > img { display:none; }
        [data-icon-tone="${tone}"][data-icon-tone] > .sugarcubes-cube-icon__contrast { display:block; }
        .sugarcubes-cube-icon__fallback { filter:${source.appearance.dark ? 'none' : 'brightness(0)'}; }
      `;
    };
    this.#unsubscribe = source.subscribeAppearance(refresh);
    refresh();
  }

  /** Release the subscription and exact owned stylesheet with the appearance host. */
  dispose(): void {
    this.#unsubscribe();
    this.#style.remove();
  }
}
