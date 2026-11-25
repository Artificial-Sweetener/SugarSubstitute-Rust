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
/**
 * The completion catalog a prompt's editor holds: one object that always searches the library as
 * it is now. The model library publishes a new catalog for every change to any model. An editor
 * given each of those would treat every one as a new capability, and prepare and draw its whole
 * prompt again. It is given this instead, and a new one only when the library's authority changes,
 * which an editor does show: names it cannot find are unknown until the library is complete.
 */
import type { LoraCompletionCatalog } from '../../../../sugarprompt/services/src/index.js';

/** The revision an editor sees for the library, whichever catalog currently answers for it. */
export const PROMPT_LIBRARY_REVISION = 'library';

/** Follow the library's catalogs and hold the one object an editor completes from. */
export class PromptCompletionCatalog {
  #current: LoraCompletionCatalog | null = null;
  #held: LoraCompletionCatalog | null = null;

  /** Search `next` from now on; the returned catalog is the same object unless authority changed. */
  follow(next: LoraCompletionCatalog | null): LoraCompletionCatalog | null {
    this.#current = next;
    if (!next) this.#held = null;
    else if (this.#held?.authoritative !== next.authoritative)
      this.#held = Object.freeze({
        revision: PROMPT_LIBRARY_REVISION,
        authoritative: next.authoritative,
        search: (query: string) => this.#current?.search(query) ?? [],
      });
    return this.#held;
  }
}
