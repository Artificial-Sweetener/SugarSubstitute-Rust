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
/** Present Substitute's independent workflow and sampler progress tracks. */
import type { WorkflowProgressSnapshot } from '../../execution/application/WorkflowProgressStore.js';

/** Preserve two compact transparent tracks with accessible progress values. */
export class GenerationProgressView {
  readonly element: HTMLDivElement;
  readonly #workflow: HTMLDivElement;
  readonly #sampler: HTMLDivElement;

  /** Build retained progress nodes without labels occupying the menu row. */
  constructor(document: Document) {
    this.element = document.createElement('div');
    this.element.className = 'sc-generation-progress';
    this.element.hidden = true;
    this.#workflow = this.#bar(document, 'workflow', 'Workflow progress');
    this.#sampler = this.#bar(document, 'sampler', 'Sampler progress');
    this.element.append(this.#workflow, this.#sampler);
  }

  /** Set widths only from the validated execution projection and retain bar identity. */
  update(snapshot: WorkflowProgressSnapshot): void {
    this.element.hidden = !snapshot.visible;
    for (const [bar, value] of [
      [this.#workflow, snapshot.workflow],
      [this.#sampler, snapshot.sampler],
    ] as const) {
      const percent = Number.isFinite(value) ? Math.max(0, Math.min(100, value)) : 0;
      bar.setAttribute('aria-valuenow', String(Math.round(percent)));
      bar.style.width = `${String(percent)}%`;
    }
  }

  /** Create one transparent source-matched track with an explicit accessible name. */
  #bar(document: Document, kind: string, label: string): HTMLDivElement {
    const bar = document.createElement('div');
    bar.className = `sc-generation-progress__${kind}`;
    bar.setAttribute('role', 'progressbar');
    bar.setAttribute('aria-label', label);
    bar.setAttribute('aria-valuemin', '0');
    bar.setAttribute('aria-valuemax', '100');
    return bar;
  }
}
