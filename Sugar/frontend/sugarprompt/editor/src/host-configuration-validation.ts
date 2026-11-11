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
/** Validate prepared external publications structurally without reconstructing service policy or catalogs. */
import { requireValue } from '../../core/src/required.js';
import type { HostConfiguration } from './host-update.js';
import { validateSavedSegmentHost } from './saved-segment-menu.js';
import { validateFieldActionUtilities } from './field-action-utilities.js';
import {
  captureConditioningContext,
  featureDefinitions,
  validatePreparedLoraMetadata,
  validatePreparedSceneQueue,
} from '../../services/src/index.js';
import { validateDocumentKind } from '../../core/src/index.js';
import { validateRichRendering } from './presentation.js';
import { validateSourceLineChrome } from './source-line-contracts.js';
import { validateWheelMode } from './wheel-mode.js';
import { captureLoraSchedulingHost } from './lora-schedule.js';
import { isSpellcheckSession } from '../../services/src/diagnostics/spelling-session.js';
/** Admit supplied configuration through its domain owners before capture, equality or native preparation. */
export function validateConfigurationValues(input: HostConfiguration): void {
  validateFieldActionUtilities(input.fieldActionUtilities);
  validatePreparedConfiguration(input);
  if ('savedSegmentHost' in input) validateSavedSegmentHost(input.savedSegmentHost);
  if ('documentKind' in input)
    validateDocumentKind(requireValue(input.documentKind, 'host-configuration-validation'));
  if ('richRendering' in input)
    validateRichRendering(requireValue(input.richRendering, 'host-configuration-validation'));
  if ('placeholder' in input && typeof input.placeholder !== 'string')
    throw new TypeError('Invalid placeholder');
  if ('readOnly' in input && typeof input.readOnly !== 'boolean')
    throw new TypeError('Invalid permission');
  if ('sourceLineChrome' in input)
    validateSourceLineChrome(requireValue(input.sourceLineChrome, 'host-configuration-validation'));
  if ('conditioningContext' in input) captureConditioningContext(input.conditioningContext);
  if ('wheelMode' in input) validateWheelMode(input.wheelMode);
  if ('loraSchedulingHost' in input)
    captureLoraSchedulingHost(
      requireValue(input.loraSchedulingHost, 'host-configuration-validation'),
    );
  if (
    'booruPostLookup' in input &&
    input.booruPostLookup !== null &&
    typeof input.booruPostLookup !== 'function'
  )
    throw new TypeError('Invalid booru post lookup');
  if (
    'spellcheckSession' in input &&
    input.spellcheckSession !== null &&
    !isSpellcheckSession(input.spellcheckSession)
  )
    throw new TypeError('Invalid spellcheck session');
}
/** Inspect an unknown record only after rejecting arrays and nonobjects. */
function record(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}
/** Prepared interfaces expose synchronous owned methods; malformed host envelopes cannot enter native facets. */
function methods(value: unknown, names: readonly string[]): boolean {
  return record(value) && names.every((name) => typeof value[name] === 'function');
}
/** Every prepared array position must be an authored own entry; iteration methods otherwise skip holes. */
function dense(values: readonly unknown[]): boolean {
  for (let index = 0; index < values.length; index++)
    if (!Object.hasOwn(values, index)) return false;
  return true;
}
/** Check public prepared shape; the existing service constructors retain validation/indexing/decision authority. */
export function validatePreparedConfiguration(input: HostConfiguration): void {
  if (input.tagLexicon != null && !methods(input.tagLexicon, ['contains']))
    throw new TypeError('Invalid tag lexicon');
  for (const key of [
    'tagCompletionCatalog',
    'loraCompletionCatalog',
    'wildcardCatalog',
    'sceneTitleCatalog',
  ] as const) {
    const catalog = input[key];
    if (
      catalog != null &&
      (!record(catalog) ||
        typeof catalog.revision !== 'string' ||
        !catalog.revision ||
        !methods(catalog, key === 'wildcardCatalog' ? ['search', 'resolve'] : ['search']))
    )
      throw new TypeError('Invalid prepared catalog');
  }
  if (input.tagCompletionCatalog && !methods(input.tagCompletionCatalog.lexicon, ['contains']))
    throw new TypeError('Invalid catalog lexicon');
  if (input.loraCompletionCatalog && typeof input.loraCompletionCatalog.authoritative !== 'boolean')
    throw new TypeError('Invalid catalog authority');
  for (const key of ['wildcardCatalog', 'sceneTitleCatalog'] as const) {
    const catalog = input[key];
    if (catalog && !['ready', 'pending', 'unavailable'].includes(catalog.status))
      throw new TypeError('Invalid catalog status');
  }
  const profile = input.featureProfile;
  if (
    'featureProfile' in input &&
    (!record(profile) ||
      !Array.isArray(profile.decisions) ||
      !dense(profile.decisions) ||
      Object.keys(profile).sort().join() !== 'decisions,rendererSyntax' ||
      profile.decisions.length !== featureDefinitions.length ||
      new Set(
        profile.decisions.map((decision: unknown) => (record(decision) ? decision.feature : null)),
      ).size !== featureDefinitions.length ||
      !Array.isArray(profile.rendererSyntax) ||
      !dense(profile.rendererSyntax) ||
      new Set(profile.rendererSyntax).size !== profile.rendererSyntax.length ||
      !profile.rendererSyntax.every(
        (syntax: unknown) => syntax === 'emphasis' || syntax === 'wildcard' || syntax === 'lora',
      ) ||
      !profile.decisions.every(
        (decision: unknown) =>
          record(decision) &&
          Object.keys(decision).sort().join() === 'detail,disabledReason,enabled,feature' &&
          featureDefinitions.some(({ id }) => id === decision.feature) &&
          (decision.enabled === true
            ? decision.disabledReason === null && decision.detail === null
            : decision.enabled === false &&
              typeof decision.disabledReason === 'string' &&
              ['user_disabled', 'field_disabled', 'missing_service', 'conflict'].includes(
                decision.disabledReason,
              ) &&
              (decision.detail === null || typeof decision.detail === 'string')),
      ))
  )
    throw new TypeError('Invalid prepared profile');
  const page = input.loraPageHost;
  if (
    page !== undefined &&
    page !== null &&
    (!methods(page, ['observe', 'report']) || !methods(page.adapter, ['open']))
  )
    throw new TypeError('Invalid model page host');
  const metadata = input.loraMetadataHost;
  if (
    metadata !== undefined &&
    metadata !== null &&
    (!methods(metadata, ['accept', 'observe', 'report']) ||
      !record(metadata.capability) ||
      typeof metadata.catalogRevision !== 'string')
  )
    throw new TypeError('Invalid metadata host');
  if (metadata !== undefined && metadata !== null) {
    if (metadata.capability === undefined) throw new TypeError('Missing metadata capability');
    validatePreparedLoraMetadata(metadata.capability);
    if (!metadata.catalogRevision) throw new TypeError('Invalid metadata revision');
  }
  const queue = input.sceneQueueHost;
  if (
    queue !== undefined &&
    queue !== null &&
    (!methods(queue, ['observe', 'report']) ||
      !record(queue.capability) ||
      !Number.isSafeInteger(queue.capability.sourceRevision))
  )
    throw new TypeError('Invalid scene queue host');
  if (queue !== undefined && queue !== null) validatePreparedSceneQueue(queue.capability);
}
