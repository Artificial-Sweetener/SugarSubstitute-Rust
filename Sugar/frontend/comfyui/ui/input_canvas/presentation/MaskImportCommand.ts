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
/** Offer "Choose mask" on a regional mask card to import pictures as new regions. */
import { CommandButtonMount } from '../../surface/appearance/CommandButtonMount.js';
import type { InputSurfaceCardPort } from '../application/InputEditorPorts.js';
import { WORKFLOW_SURFACE_COPY } from '../application/WorkflowSurfaceCopy.js';

/** Show the command only while the loader's masks live on a workflow-surface canvas. */
export class MaskImportCommand {
  readonly element: HTMLElement;
  readonly #button: CommandButtonMount;
  readonly #input: HTMLInputElement;
  readonly #stop: () => void;
  #busy = false;
  #alive = true;

  /** Bind the command to one loader card. */
  constructor(
    document: Document,
    private readonly surface: InputSurfaceCardPort,
    private readonly report: (error: unknown) => void,
  ) {
    this.element = document.createElement('span');
    this.element.className = 'sc-input-editor__import';
    this.#input = document.createElement('input');
    this.#input.type = 'file';
    this.#input.accept = 'image/png,image/jpeg,image/bmp,image/gif,image/webp';
    this.#input.multiple = true;
    this.#input.hidden = true;
    this.#input.setAttribute('aria-label', WORKFLOW_SURFACE_COPY.chooseMaskFiles);
    this.#input.addEventListener('change', this.#chosen);
    this.#button = new CommandButtonMount(document, WORKFLOW_SURFACE_COPY.chooseMask, () => {
      if (!this.#busy && this.surface.canImport()) this.#input.click();
    });
    this.element.append(this.#button.element, this.#input);
    this.#stop = surface.subscribe(() => this.refresh());
    this.refresh();
  }

  /** Reflect availability and a running import. */
  refresh(): void {
    if (!this.#alive) return;
    const available = this.surface.canImport();
    this.element.hidden = !available;
    this.#button.setDisabled(this.#busy || !available);
  }

  /** Release the command and ignore a running import's completion. */
  dispose(): void {
    this.#alive = false;
    this.#stop();
    this.#input.removeEventListener('change', this.#chosen);
    this.#button.dispose();
    this.element.remove();
  }

  /** Import every chosen picture in order, as one operation. */
  readonly #chosen = (): void => {
    const files = [...(this.#input.files ?? [])];
    this.#input.value = '';
    if (!files.length || this.#busy) return;
    this.#busy = true;
    this.refresh();
    this.surface
      .importMasks(files)
      .catch((error: unknown) => {
        if (this.#alive) this.report(error);
      })
      .finally(() => {
        this.#busy = false;
        this.refresh();
      });
  };
}
