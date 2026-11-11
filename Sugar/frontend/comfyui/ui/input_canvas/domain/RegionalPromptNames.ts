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
/** Keep the separator names of the prompts related to one regional mask loader identical. */
import {
  applyRegionNames,
  changedRegionNames,
  parsePrompt,
  type RegionName,
} from '../../../../sugarprompt/core/src/index.js';
import { resolveSharedRegionNames } from '../../../../sugarprompt/services/src/index.js';

/** One related prompt's current text and, when an edit was observed in it, its text before. */
export interface RegionalPromptSource {
  readonly text: string;
  readonly previous: string | null;
}

/**
 * Give separator `i` of every related prompt the same name.
 *
 * Names a prompt edit explicitly changed, including clearing one, win; every other ordinal takes
 * the first non-blank name in prompt order. Separators are never added or removed, and a prompt
 * without separator `i` is left unchanged there. When several prompts were edited, the later
 * prompt's change to an ordinal wins.
 *
 * @returns The rewritten text of each prompt, or null where it is already consistent.
 */
export function synchronizeRegionalPromptNames(
  prompts: readonly RegionalPromptSource[],
): readonly (string | null)[] {
  const documents = prompts.map((prompt) => parsePrompt(prompt.text));
  const changes = new Map<number, RegionName>();
  for (const [index, prompt] of prompts.entries()) {
    const document = documents[index];
    if (prompt.previous === null || !document) continue;
    for (const change of changedRegionNames(parsePrompt(prompt.previous), document))
      changes.set(change.ordinal, change);
  }
  const names = resolveSharedRegionNames(documents, [...changes.values()]);
  return documents.map((document) => {
    const application = applyRegionNames(document, names);
    return application.replacements.length ? application.source : null;
  });
}
