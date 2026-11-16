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
/** Resolve the wildcards of one queued run's prompt texts, as SugarSubstitute's Python app did. */
import {
  prepareWildcardGeneration,
  type WildcardCandidateSource,
} from '../../../../sugarprompt/services/src/index.js';

/** The part of the wildcard library a run reads. */
export interface WildcardCandidates {
  readonly revision: string;
  refresh(): Promise<void>;
  candidates(form: 'simple' | 'csv', name: string): Promise<WildcardCandidateSource | null>;
}

/** Most rounds of reading newly named wildcards; a candidate may itself name wildcards. */
const ROUNDS = 25;

/**
 * Resolve every wildcard in a run's prompt texts with one seed.
 *
 * All texts share one pass, so a wildcard named in several fields picks the same candidate in
 * each. Only the wildcards a text names are read, including those a picked candidate names in
 * turn. A name the folder does not hold stays in the text as written. A text with no opening
 * brace cannot name a wildcard, so a run without any reads nothing.
 *
 * @param seed The run's seed as a decimal integer, or null for a run that has none.
 * @param entropy A fresh decimal integer used in place of a missing seed.
 * @returns Each text that changed, mapped to its resolved text.
 */
export async function resolveWildcardTexts(
  texts: readonly string[],
  seed: string | null,
  entropy: string,
  library: WildcardCandidates,
): Promise<ReadonlyMap<string, string>> {
  const sources = [...new Set(texts)].filter((text) => text.includes('{'));
  if (!sources.length) return new Map();
  await library.refresh();
  const loaded = new Map<string, WildcardCandidateSource>();
  const asked = new Set<string>();
  for (let round = 0; ; round++) {
    const preparation = prepareWildcardGeneration({
      revision: library.revision,
      sources: [...loaded.values()],
      seed,
      ...(seed === null ? { entropySeed: entropy } : {}),
    });
    if (preparation.status !== 'ready') throw new Error('Wildcard candidates could not be read.');
    const resolved = new Map<string, string>();
    const wanted = new Map<string, { form: 'simple' | 'csv'; identifier: string }>();
    try {
      for (const source of sources) {
        const outcome = preparation.pass.resolve(source);
        if (outcome.status === 'rejected' || outcome.status === 'disabled') continue;
        if (outcome.resolvedText !== source) resolved.set(source, outcome.resolvedText);
        for (const issue of outcome.issues) {
          const name = issue.form + ':' + issue.identifier;
          if (issue.reason === 'source-missing' && !asked.has(name))
            wanted.set(name, { form: issue.form, identifier: issue.identifier });
        }
      }
    } finally {
      preparation.pass.dispose();
    }
    let added = false;
    if (round < ROUNDS)
      for (const [name, { form, identifier }] of wanted) {
        asked.add(name);
        const candidates = await library.candidates(form, identifier);
        // Two spellings of one name read the same wildcard; it is offered to the pass once.
        const held = candidates ? form + ':' + candidates.identifier : null;
        if (!candidates || held === null || loaded.has(held)) continue;
        loaded.set(held, candidates);
        added = true;
      }
    if (!added) return resolved;
  }
}
