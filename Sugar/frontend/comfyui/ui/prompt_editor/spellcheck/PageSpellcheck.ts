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
/** The page's one spellchecker, whose gateway every prompt editor's spelling session asks. */
import type { SpellcheckGateway } from '../../../../sugarprompt/services/src/index.js';
import { ComfySettingStore } from '../../settings/workspace/ComfySettingStore.js';
import { WorkerSpellcheckGateway, type PersonalSpellingWords } from './WorkerSpellcheckGateway.js';

/** The ComfyUI setting that keeps the words the person added to the dictionary. */
export const SPELLING_WORDS_SETTING = 'SugarCubes.PromptEditor.SpellingWords';

/** The added words, kept in ComfyUI's settings. */
class ComfySpellingWords implements PersonalSpellingWords {
  readonly #store: ComfySettingStore;

  constructor(store: ComfySettingStore) {
    this.#store = store;
  }

  read(): readonly string[] {
    const stored = this.#store.get(SPELLING_WORDS_SETTING);
    return Array.isArray(stored) ? stored.filter((word) => typeof word === 'string') : [];
  }

  async add(word: string): Promise<void> {
    const words = this.read();
    if (!words.includes(word)) await this.#store.set(SPELLING_WORDS_SETTING, [...words, word]);
  }
}

/**
 * Start the spellchecker for this page, or null where no Web Worker can run. The worker and the
 * files it reads are deployed beside this module; the dictionary downloads on the first check.
 */
export function startPageSpellcheck(
  app: unknown,
  report: (error: unknown) => void,
): SpellcheckGateway | null {
  if (typeof Worker === 'undefined') return null;
  const store = new ComfySettingStore(app);
  store.register({
    id: SPELLING_WORDS_SETTING,
    name: 'Prompt editor dictionary words',
    type: 'hidden',
    defaultValue: [],
  });
  const deployed = (path: string): string => new URL(path, import.meta.url).href;
  const worker = new Worker(deployed('./SpellcheckWorker.js'), {
    type: 'module',
    name: 'SugarCubes spellchecker',
  });
  return new WorkerSpellcheckGateway(
    worker,
    {
      module: deployed('../../../../spellcheck/sugarcubes-spellcheck.wasm'),
      aff: deployed('../../../../spellcheck/en_US.aff'),
      dic: deployed('../../../../spellcheck/en_US.dic'),
    },
    new ComfySpellingWords(store),
    report,
  );
}
