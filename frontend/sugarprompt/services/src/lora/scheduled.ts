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
/** Effective schedules consume prepared core syntax and validated resource/context snapshots, never a second source parser. */
import { strip } from '../../../core/src/prompt/text.js';
import type { LoraCatalogItem, LoraResourceSnapshot } from './contracts.js';
import { loraNameKey } from './names.js';
import { assertPreparedLoraSchedulingContext } from './scheduled-context.js';
import type {
  EffectiveLoraSnapshot,
  PreparedLoraSchedulingContext,
  ScheduledLora,
  EffectiveLoraModelSnapshot,
} from './scheduled-contracts.js';
import { prepareLoraTriggerIndex } from './trigger-index.js';
import { readLoraResources } from './validation.js';

/** First effective model owns canonical metadata; configured whole entries remain intact for explicit insertion. */
function scheduled(item: LoraCatalogItem, source: ScheduledLora['source']): ScheduledLora {
  const title = strip(item.title) || strip(item.model.basename),
    subtitle = item.subtitle === null ? '' : strip(item.subtitle);
  return Object.freeze({
    model: item.model,
    display: title && subtitle ? `${title} - ${subtitle}` : title || subtitle,
    triggerWords: item.triggerWords,
    source,
    insertionText: item.triggerWords
      .map((word) => strip(word))
      .filter(Boolean)
      .join(', '),
  });
}

/** One preparation pass preserves inline → upstream → field order and normalized physical identity.
 * The caller supplies exact ordered names from the core-parsed effective prompt; scene composition remains core-owned.
 * Resources must be the ready, exact requested-name snapshot for that scope. Missing/ambiguous models supply no triggers.
 */
export function prepareEffectiveLoras(
  names: readonly string[],
  resources: LoraResourceSnapshot,
  catalogRevision: string | null,
  context: PreparedLoraSchedulingContext | null,
): EffectiveLoraSnapshot {
  const prepared = prepareEffectiveLoraModels(names, resources, catalogRevision, context);
  return prepared.status === 'ready'
    ? Object.freeze({
        ...prepared,
        triggers: prepareLoraTriggerIndex(prepared.models),
      })
    : prepared;
}

/** The sole effective-order and first-metadata owner also supplies cheap scene-specific action eligibility. */
export function prepareEffectiveLoraModels(
  names: readonly string[],
  resources: LoraResourceSnapshot,
  catalogRevision: string | null,
  context: PreparedLoraSchedulingContext | null,
): EffectiveLoraModelSnapshot {
  if (context === null) return Object.freeze({ status: 'unavailable', reason: 'missing-context' });
  assertPreparedLoraSchedulingContext(context);
  if (!context.supported)
    return Object.freeze({
      status: 'unavailable',
      reason: 'unsupported-scheduling',
    });
  if (catalogRevision !== context.catalogRevision)
    return Object.freeze({ status: 'unavailable', reason: 'stale-catalog' });
  if (resources.status !== 'ready')
    return Object.freeze({
      status: 'unavailable',
      reason: 'resources-unavailable',
    });
  const resolved = readLoraResources(resources.resources, names),
    models: ScheduledLora[] = [],
    seen = new Set<string>();
  /** Deduplicate once across all origins; canonical metadata is never inferred from an authored alias. */
  function add(item: LoraCatalogItem, source: ScheduledLora['source']): void {
    const key = loraNameKey(
      strip(item.model.backendValue) ? item.model.backendValue : item.model.promptName,
    );
    if (seen.has(key)) return;
    seen.add(key);
    models.push(scheduled(item, source));
  }
  for (const item of resolved) if (item.resolution === 'found') add(item, 'inline');
  for (const item of context.upstream) add(item, 'upstream');
  for (const item of context.fields) add(item, 'field');
  const captured = Object.freeze(models);
  return Object.freeze({
    status: 'ready',
    context,
    models: captured,
  });
}
