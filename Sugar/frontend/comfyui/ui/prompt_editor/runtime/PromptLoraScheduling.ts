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
/** Tell one prompt editor which LoRAs shape its prompt besides the ones written in it. */
import type { PromptEditorApi } from '../../../../sugarprompt/editor/src/contracts.js';
import { prepareLoraSchedulingContext } from '../../../../sugarprompt/services/src/index.js';
import type { LoraCatalogItem } from '../../../../sugarprompt/services/src/index.js';
import { PROMPT_LIBRARY_REVISION } from './PromptCompletionCatalog.js';

/**
 * Publish the models loaded ahead of a prompt's encoder.
 *
 * Their trigger words are offered as words while typing and from the prompt's menu. Nothing is
 * published again while those models, their names and their words are unchanged: an editor
 * prepares its prompt again for every context it is given, and the library publishes a new
 * catalog for every change to any model.
 */
export class PromptLoraScheduling {
  #key = '';
  #revision = 0;

  /** Bind the editor, whether its prompt loads written LoRAs, and the host's upstream lookup. */
  constructor(
    private readonly api: Pick<PromptEditorApi, 'setLoraSchedulingHost'>,
    private readonly supported: boolean,
    private readonly upstream: () => readonly LoraCatalogItem[],
  ) {}

  /** Publish when the upstream models, or what the library knows of them, changed. */
  publish(): void {
    const upstream = this.upstream();
    const key = JSON.stringify(
      upstream.map((item) => [item.model.backendValue, item.title, item.triggerWords]),
    );
    if (key === this.#key) return;
    this.#key = key;
    this.api.setLoraSchedulingHost({
      context: prepareLoraSchedulingContext({
        revision: `${PROMPT_LIBRARY_REVISION}#${String(++this.#revision)}`,
        catalogRevision: PROMPT_LIBRARY_REVISION,
        mode: 'workflow',
        supported: this.supported,
        upstream,
        fields: [],
      }),
      catalogRevision: PROMPT_LIBRARY_REVISION,
    });
  }
}
