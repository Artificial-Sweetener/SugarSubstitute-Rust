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
/** Offer "Browse Files" on a scalar mask card to load a picture into its input-canvas mask. */
import { CommandButtonMount } from '../../surface/appearance/CommandButtonMount.js';
import { WORKFLOW_SURFACE_COPY } from '../application/WorkflowSurfaceCopy.js';

/** Upload one chosen picture and hand its Comfy file to the owner that loads it into the mask. */
export class MaskFileCommand {
  readonly element: HTMLElement;
  readonly #button: CommandButtonMount;
  readonly #input: HTMLInputElement;
  #busy = false;
  #disabled = false;
  #alive = true;

  /**
   * `load` receives the uploaded file; the surface provisioning compares its size with the
   * mask's image and reports a mismatch instead of loading it.
   */
  constructor(
    document: Document,
    private readonly upload: (file: File) => Promise<string>,
    private readonly load: (file: string) => void,
    private readonly report: (error: unknown) => void,
  ) {
    this.element = document.createElement('span');
    this.element.className = 'sc-input-editor__browse';
    this.#input = document.createElement('input');
    this.#input.type = 'file';
    this.#input.accept = 'image/png,image/jpeg,image/bmp,image/gif,image/webp';
    this.#input.hidden = true;
    this.#input.setAttribute('aria-label', WORKFLOW_SURFACE_COPY.chooseMaskFile);
    this.#input.addEventListener('change', this.#chosen);
    this.#button = new CommandButtonMount(document, WORKFLOW_SURFACE_COPY.browseFiles, () => {
      if (!this.#busy && !this.#disabled) this.#input.click();
    });
    this.element.append(this.#button.element, this.#input);
  }

  /** Show the command only where the card presents its input-canvas mask. */
  update(visible: boolean, disabled: boolean): void {
    this.element.hidden = !visible;
    this.#disabled = disabled || !visible;
    this.#button.setDisabled(this.#busy || this.#disabled);
  }

  /** Release the command and ignore a running upload's completion. */
  dispose(): void {
    this.#alive = false;
    this.#input.removeEventListener('change', this.#chosen);
    this.#button.dispose();
    this.element.remove();
  }

  /** Upload the chosen picture once, then load it unless the card retired meanwhile. */
  readonly #chosen = (): void => {
    const file = this.#input.files?.[0];
    this.#input.value = '';
    if (!file || this.#busy) return;
    this.#busy = true;
    this.#button.setDisabled(true);
    this.upload(file)
      .then((path) => {
        if (this.#alive) this.load(path);
      })
      .catch((error: unknown) => {
        if (this.#alive) this.report(error);
      })
      .finally(() => {
        this.#busy = false;
        if (this.#alive) this.#button.setDisabled(this.#disabled);
      });
  };
}
