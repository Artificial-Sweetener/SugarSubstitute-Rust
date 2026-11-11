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
/** Present Cube pack provenance in Comfy's native node-picker result details. */

import type { ComfyCubeNodeDefinition } from './ComfyCubeNodeDefProjector.js';
import { ComfyCubePickerModelTitlePresenter } from './ComfyCubePickerModelTitlePresenter.js';
import {
  ComfyCubePickerPackPresenter,
  PRESENTED_PACK_TYPE_ATTRIBUTE,
} from './ComfyCubePickerPackPresenter.js';

const RESULT_SELECTOR = '[data-testid="result-item"]';
const RESULTS_LIST_SELECTOR = '#results-list';

export interface ComfyCubePickerResultPresenterOptions {
  document: Document;
  definitions(): readonly ComfyCubeNodeDefinition[];
}

/** Adapt only the display label Comfy couples to its category navigation field. */
export class ComfyCubePickerResultPresenter {
  readonly #document: Document;
  readonly #definitions: () => readonly ComfyCubeNodeDefinition[];
  readonly #packs = new ComfyCubePickerPackPresenter();
  readonly #modelTitles = new ComfyCubePickerModelTitlePresenter();
  #installed = false;
  #frame: number | null = null;
  #observedResults: Element | null = null;
  #resultsObserver: MutationObserver | null = null;

  /** Bind the current document and dynamic native-definition snapshot. */
  constructor(options: ComfyCubePickerResultPresenterOptions) {
    this.#document = options.document;
    this.#definitions = options.definitions;
  }

  /** Start activity-driven reconciliation without observing the entire application body. */
  install(): void {
    if (this.#installed) return;
    this.#installed = true;
    this.#document.addEventListener('pointerdown', this.schedule, true);
    this.#document.addEventListener('keydown', this.schedule, true);
    this.#document.addEventListener('input', this.schedule, true);
    this.schedule();
  }

  /** Stop all document and picker-local presentation work. */
  dispose(): void {
    if (!this.#installed) return;
    this.#installed = false;
    this.#document.removeEventListener('pointerdown', this.schedule, true);
    this.#document.removeEventListener('keydown', this.schedule, true);
    this.#document.removeEventListener('input', this.schedule, true);
    const windowRef = this.#document.defaultView;
    if (this.#frame !== null && windowRef) windowRef.cancelAnimationFrame(this.#frame);
    this.#frame = null;
    this.#resultsObserver?.disconnect();
    this.#resultsObserver = null;
    this.#observedResults = null;
  }

  /** Reconcile after Comfy renders or changes its native picker results. */
  schedule = (): void => {
    if (!this.#installed || this.#frame !== null) return;
    const windowRef = this.#document.defaultView;
    if (!windowRef) return;
    this.#frame = windowRef.requestAnimationFrame(() => {
      this.#frame = null;
      this.refresh();
    });
  };

  /** Replace only uniquely identified Cube category labels with pack provenance. */
  refresh(): void {
    this.#observeCurrentResults();
    const definitions = this.#definitions();
    for (const result of this.#document.querySelectorAll<HTMLElement>(RESULT_SELECTOR)) {
      this.#presentResult(result, definitions);
    }
  }

  /** Restrict mutation reconciliation to Comfy's mounted result list. */
  #observeCurrentResults(): void {
    const results = this.#document.querySelector(RESULTS_LIST_SELECTOR);
    if (results === this.#observedResults) return;
    this.#resultsObserver?.disconnect();
    this.#resultsObserver = null;
    this.#observedResults = results;
    const MutationObserverType = this.#document.defaultView?.MutationObserver;
    if (!results || !MutationObserverType) return;
    this.#resultsObserver = new MutationObserverType(() => this.refresh());
    this.#resultsObserver.observe(results, { childList: true, characterData: true, subtree: true });
  }

  /** Present one result only when its definition signature resolves unambiguously. */
  #presentResult(result: HTMLElement, definitions: readonly ComfyCubeNodeDefinition[]): void {
    const candidates = resolveResultDefinitions(result, definitions);
    if (candidates.length !== 1) return;
    const definition = candidates[0];
    if (!definition) return;
    this.#packs.present(result, definition);
    this.#modelTitles.present(result, definition);
  }
}

/** Resolve a result through an existing claim or its exact visible definition signature. */
function resolveResultDefinitions(
  result: HTMLElement,
  definitions: readonly ComfyCubeNodeDefinition[],
): ComfyCubeNodeDefinition[] {
  const claimedType = result
    .querySelector<HTMLElement>(`[${PRESENTED_PACK_TYPE_ATTRIBUTE}]`)
    ?.getAttribute(PRESENTED_PACK_TYPE_ATTRIBUTE);
  if (claimedType) return definitions.filter(({ name }) => name === claimedType);
  const resultText = result.textContent?.trim() ?? '';
  if (!resultText) return [];
  return definitions.filter(
    (definition) =>
      resultText.startsWith(definition.display_name) &&
      (!definition.description || resultText.includes(definition.description)),
  );
}
