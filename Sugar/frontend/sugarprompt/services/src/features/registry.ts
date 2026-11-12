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
/** Retained feature registry in donor priority order; settings and decisions share this owner. */
import type { FeatureDefinition, FeatureId } from './contracts.js';

/** Construct frozen registry metadata; all retained capabilities default to allowed. */
function definition(
  id: FeatureId,
  label: string,
  aliases: readonly string[],
  syntax: FeatureDefinition['syntax'] = null,
  dependencies: readonly FeatureId[] = [],
  legacyBase = false,
): FeatureDefinition {
  return Object.freeze({
    id,
    label,
    aliases: Object.freeze(aliases),
    syntax,
    dependencies: Object.freeze(dependencies),
    conflicts: Object.freeze([]),
    legacyBase,
  });
}

/** Twelve retained capabilities; wiki lookup has no registry entry or settings control. */
export const featureDefinitions: readonly FeatureDefinition[] = Object.freeze([
  definition('emphasis', 'Emphasis weights', ['emphasis'], 'emphasis'),
  definition('danbooru_url_import', 'Danbooru URL import', ['danbooru_url_import'], null, [], true),
  definition('wildcard_syntax', 'Wildcard syntax', ['wildcard', 'wildcard_syntax'], 'wildcard'),
  definition(
    'wildcard_autocomplete',
    'Wildcard autocomplete',
    ['wildcard', 'wildcard_autocomplete'],
    null,
    ['wildcard_syntax'],
  ),
  definition('autocomplete_ghost_text', 'Autocomplete ghost text', [], null, [], true),
  definition('lora_syntax', 'LoRA syntax', ['lora', 'lora_syntax'], 'lora'),
  definition('lora_autocomplete', 'LoRA autocomplete', ['lora', 'lora_autocomplete'], null, [
    'lora_syntax',
  ]),
  definition('lora_picker', 'LoRA picker', ['lora', 'lora_picker'], null, ['lora_syntax']),
  definition('lora_trigger_words', 'LoRA trigger words', ['lora', 'lora_trigger_words']),
  definition('segment_reorder', 'Segment reorder', ['segment_reorder'], null, [], true),
  definition('spellcheck', 'Spellcheck', ['spellcheck'], null, [], true),
  definition(
    'duplicate_segment_diagnostics',
    'Duplicate segment warnings',
    ['duplicate_segment_diagnostics'],
    null,
    [],
    true,
  ),
]);

/** Validate a feature identifier at unknown host data boundaries. */
export function isFeatureId(value: unknown): value is FeatureId {
  return featureDefinitions.some((entry) => entry.id === value);
}
