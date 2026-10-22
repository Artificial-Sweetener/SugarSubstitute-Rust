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
/** Actual canonical caret scopes supply decoded completion queries and genuine encoded acceptance outside input. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState } from '@codemirror/state';
import { tagGhostText } from '../../core/src/index.js';
import { prepareTagReplacement } from '../../core/src/completion/text.js';
import {
  featureEnabled,
  findPromptTextCompletions,
  resolveCompletionQuery,
} from '../../services/src/index.js';
import { editorFreshness } from './freshness.js';
import {
  tagCompletionCatalog,
  loraCompletionCatalog,
  sceneTitleCatalog,
  type PreparedCompletion,
  type PreparedCompletionResult,
  type PreparedCompletionEdit,
} from './prompt-completion-model.js';
import { featureProfile } from './features.js';
import { loraCompletionOptions } from './lora-completion-options.js';
import { stableTagCompletionPrefix } from '../../core/src/completion/continuation.js';
import {
  completionPosition,
  prepareCompletionScope,
  prepareScopedCompletionEdit,
} from './completion-splice.js';
import { nativeLoraSchedule } from './lora-schedule.js';
import { wildcardCatalog } from './wildcard-catalog.js';
import { wildcardCompletionOptions } from './wildcard-completion-options.js';
import { sceneCompletionOptions } from './scene-completion-options.js';
/** Resolve one admitted decoded caret once, retaining syntax priority and every ranked alternative. */
export function prepareCompletionOptions(state: EditorState): PreparedCompletionResult | null {
  const scope = prepareCompletionScope(state);
  if (!scope) return null;
  const caret = state.selection.main.head,
    source = scope.target.scope.mapping.text,
    decodedCaret = scope.target.decodedRange.from,
    routed = resolveCompletionQuery(
      scope.target.scope,
      state.facet(featureProfile),
      decodedCaret,
      false,
    );
  if (!routed) return null;
  if (routed.kind === 'lora') {
    const catalog = state.facet(loraCompletionCatalog);
    return catalog ? loraCompletionOptions(state, scope, routed.query, caret, catalog) : null;
  }
  if (routed.kind === 'wildcard') {
    const catalog = state.facet(wildcardCatalog);
    return catalog ? wildcardCompletionOptions(state, scope, routed.query, caret, catalog) : null;
  }
  if (routed.kind === 'scene') {
    const catalog = state.facet(sceneTitleCatalog);
    return catalog ? sceneCompletionOptions(state, scope, routed.query, caret, catalog) : null;
  }
  const catalog = state.facet(tagCompletionCatalog),
    exactCaret = requireValue(
      scope.target.scope.mapping.decodedToSource[decodedCaret],
      'completion-options',
    ),
    schedule = state.field(nativeLoraSchedule, false)?.scopes.at(exactCaret)?.snapshot,
    triggers =
      schedule?.status === 'ready' &&
      featureEnabled(state.facet(featureProfile), 'lora_trigger_words')
        ? schedule.triggers
        : null;
  if (!catalog && !triggers) return null;
  const result = findPromptTextCompletions(catalog, triggers, source, routed.query),
    from = completionPosition(scope, result.query.replacementRange.from),
    to = completionPosition(scope, result.query.activeTagEnd);
  if (!result.matches.length || from === null || to === null) return null;
  const stablePrefix = stableTagCompletionPrefix(
      source.slice(result.query.replacementRange.from, decodedCaret),
    ),
    options: PreparedCompletion[] = [];
  for (const row of result.matches) {
    const word = row.kind === 'tag' ? row.value.tag : row.value.word;
    /** Activation-specific delimiter policy delegates decoded transformation and native publication to their owners. */
    const acceptance = (addComma: boolean): PreparedCompletionEdit | null => {
      const prepared = prepareTagReplacement(source, result.query, word, addComma),
        retained = prepareScopedCompletionEdit(
          state,
          requireValue(scope, 'completion-options'),
          prepared.retainRight.range,
          prepared.retainRight.text,
          stablePrefix ? { from: result.query.replacementRange.from, to: decodedCaret } : null,
          { lexicon: catalog?.lexicon },
        ),
        consumed = prepared.right
          ? prepareScopedCompletionEdit(
              state,
              requireValue(scope, 'completion-options'),
              prepared.right.replacement.range,
              prepared.right.replacement.text,
              stablePrefix
                ? {
                    from: result.query.replacementRange.from,
                    to: result.query.activeTagEnd,
                  }
                : null,
              { lexicon: catalog?.lexicon },
            )
          : null;
      if (!retained || (prepared.right && !consumed)) return null;
      const selected =
        consumed && prepared.right?.match.consumesVisiblePrefix(result.query.prefix)
          ? consumed
          : retained;
      return Object.freeze({
        ...selected,
        splice: retained.splice,
        right:
          stablePrefix && consumed?.splice && prepared.right
            ? Object.freeze({
                match: prepared.right.match,
                splice: consumed.splice,
              })
            : null,
      });
    };
    const keyboard = acceptance(true),
      pointer = acceptance(false);
    if (!keyboard || !pointer) continue;
    options.push(
      Object.freeze({
        ...(row.kind === 'tag'
          ? { kind: 'tag' as const }
          : { kind: 'trigger' as const, backendValue: row.value.backendValue }),
        label: word,
        detail:
          row.kind === 'trigger'
            ? row.value.display
            : (row.value.sourceLabel ??
              (row.value.popularity === null ? '' : row.value.popularity.toLocaleString('en-US'))),
        freshness: state.field(editorFreshness),
        caret,
        ghost: tagGhostText(source, result.query, word),
        acceptance: Object.freeze({ keyboard, pointer }),
      }),
    );
  }
  return options.length
    ? Object.freeze({
        from,
        to: caret,
        query: Object.freeze({
          scope: scope.target.scope,
          kind: 'tag',
          from,
          contentFrom: from,
          to,
        }),
        options: Object.freeze(options),
      })
    : null;
}
