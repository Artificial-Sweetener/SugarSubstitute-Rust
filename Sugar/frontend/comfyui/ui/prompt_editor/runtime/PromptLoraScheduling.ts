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
import type {
  LoraCatalogItem,
  LoraResourceAdapter,
} from '../../../../sugarprompt/services/src/index.js';

/**
 * Publish the models loaded ahead of a prompt's encoder for the current catalog.
 *
 * Their trigger words are offered as words while typing and from the prompt's menu. Nothing is
 * published again while the catalog and those models are unchanged.
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

  /** Forget what was published, so the next catalog is always published. */
  reset(): void {
    this.#key = '';
  }

  /** Publish for this catalog when it or the upstream models changed. */
  publish(adapter: LoraResourceAdapter): void {
    const upstream = this.upstream();
    const key = [adapter.revision, ...upstream.map((item) => item.model.backendValue)].join('\n');
    if (key === this.#key) return;
    this.#key = key;
    this.api.setLoraSchedulingHost({
      context: prepareLoraSchedulingContext({
        revision: `${adapter.revision}#${String(++this.#revision)}`,
        catalogRevision: adapter.revision,
        mode: 'workflow',
        supported: this.supported,
        upstream,
        fields: [],
      }),
      catalogRevision: adapter.revision,
    });
  }
}
