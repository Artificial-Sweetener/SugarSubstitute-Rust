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
/** Coordinate native panel exclusion and its disabled affordance for masonry presentation. */
import { ComfyPropertiesPanelAdapter } from '../ComfyPropertiesPanelAdapter.js';
import { WorkflowMasonryPropertiesButton } from './WorkflowMasonryPropertiesButton.js';

/** Keep Properties state and presentation under one masonry lifecycle policy. */
export class WorkflowMasonryPropertiesPanel {
  readonly #panel: ComfyPropertiesPanelAdapter;
  readonly #button: WorkflowMasonryPropertiesButton;

  /** Bind the native setting adapter and focused button presenter. */
  constructor(documentRef: Document, app: unknown, reportError: (error: unknown) => void) {
    this.#panel = new ComfyPropertiesPanelAdapter(app, reportError);
    this.#button = new WorkflowMasonryPropertiesButton(documentRef);
  }

  /** Observe native affordances and lease the authoritative setting constraint. */
  setup(): void {
    this.#panel.setup();
    this.#button.setup();
  }

  /** Close Properties before masonry measurements and keep every opening route unavailable. */
  update(presenting: boolean): boolean {
    const accepted = this.#panel.update(presenting);
    this.#button.update(presenting);
    return accepted;
  }

  /** Release the constraint and restore the user's graph sidebar state and affordances. */
  dispose(): void {
    this.#panel.dispose();
    this.#button.dispose();
  }
}
