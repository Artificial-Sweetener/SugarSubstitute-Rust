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
/** Deterministic field/user/dependency/capability/conflict decisions, independent of frameworks. */
import { requireValue } from '../../../core/src/required.js';
import type {
  FeatureContext,
  FeatureDecision,
  FeatureDefinition,
  FeatureId,
  FeaturePreferences,
  FeatureProfile,
} from './contracts.js';
import { featureDefinitions, isFeatureId } from './registry.js';
/** Retain actual resolved output when host context identity changes without changing any policy result. */
export function retainFeatureProfile(
  previous: FeatureProfile,
  next: FeatureProfile,
): FeatureProfile {
  return previous.rendererSyntax.length === next.rendererSyntax.length &&
    previous.rendererSyntax.every((syntax, index) => syntax === next.rendererSyntax[index]) &&
    previous.decisions.length === next.decisions.length &&
    previous.decisions.every((decision, index) => {
      const candidate = requireValue(next.decisions[index], 'profile');
      return (
        decision.feature === candidate.feature &&
        decision.enabled === candidate.enabled &&
        decision.disabledReason === candidate.disabledReason &&
        decision.detail === candidate.detail
      );
    })
    ? previous
    : next;
}
/** Resolve retained explicit metadata or legacy aliases; unknown string entries grant no capability. */
function fieldAllows(context: FeatureContext): ReadonlySet<FeatureId> {
  const field = context.field;
  if (field?.prompt_features)
    return new Set(
      field.prompt_features.flatMap((entry) => {
        const id = typeof entry === 'string' ? entry.trim().toLowerCase() : entry;
        return isFeatureId(id) ? [id] : [];
      }),
    );
  if (field?.prompt_syntaxes) {
    const aliases = field.prompt_syntaxes
      .filter((entry): entry is string => typeof entry === 'string')
      .map((entry) => entry.trim().toLowerCase());
    return new Set(
      featureDefinitions
        .filter(
          (entry) => entry.legacyBase || entry.aliases.some((alias) => aliases.includes(alias)),
        )
        .map(({ id }) => id),
    );
  }
  return new Set(featureDefinitions.map(({ id }) => id));
}
/** Check typed host declarations at the runtime boundary before capability adaptation. */
export function validateFeatureContext(context: FeatureContext): void {
  if (
    !context ||
    typeof context !== 'object' ||
    Array.isArray(context) ||
    Object.keys(context).some(
      (key) => !['mode', 'field', 'loraScheduling', 'unavailableFeatures'].includes(key),
    ) ||
    !['library', 'workflow'].includes(context.mode) ||
    ('loraScheduling' in context && typeof context.loraScheduling !== 'boolean') ||
    ('unavailableFeatures' in context &&
      (!Array.isArray(context.unavailableFeatures) ||
        Array.from(context.unavailableFeatures).some((id) => !isFeatureId(id))))
  )
    throw new TypeError('Invalid feature context');
  if (
    'field' in context &&
    (!context.field ||
      typeof context.field !== 'object' ||
      Array.isArray(context.field) ||
      Object.keys(context.field).some(
        (key) => !['prompt_features', 'prompt_syntaxes'].includes(key),
      ) ||
      ['prompt_features', 'prompt_syntaxes'].some(
        (key) =>
          key in requireValue(context.field, 'profile') &&
          !Array.isArray(requireValue(context.field, 'profile')[key as keyof typeof context.field]),
      ))
  )
    throw new TypeError('Invalid field metadata');
}
/** Capture validated host declarations by value; unknown metadata entries remain literal and confer no additional authority. */
export function captureFeatureContext(context: FeatureContext): FeatureContext {
  validateFeatureContext(context);
  return Object.freeze({
    ...context,
    ...(context.field
      ? {
          field: Object.freeze({
            ...context.field,
            ...(context.field.prompt_features
              ? {
                  prompt_features: Object.freeze([...context.field.prompt_features]),
                }
              : {}),
            ...(context.field.prompt_syntaxes
              ? {
                  prompt_syntaxes: Object.freeze([...context.field.prompt_syntaxes]),
                }
              : {}),
          }),
        }
      : {}),
    ...(context.unavailableFeatures
      ? { unavailableFeatures: Object.freeze([...context.unavailableFeatures]) }
      : {}),
  });
}
/** Compare captured declarations, independently of resolved policy equivalence or host object identity. */
export function sameFeatureContext(left: FeatureContext, right: FeatureContext): boolean {
  /** Compare literal captured declarations without reimplementing their feature policy. */
  const sameEntries = (
    a: readonly unknown[] | undefined,
    b: readonly unknown[] | undefined,
  ): boolean =>
    a === b ||
    (a !== undefined &&
      b !== undefined &&
      a.length === b.length &&
      a.every((value, index) => Object.is(value, b[index])));
  return (
    left.mode === right.mode &&
    left.loraScheduling === right.loraScheduling &&
    (left.field === undefined) === (right.field === undefined) &&
    sameEntries(left.field?.prompt_features, right.field?.prompt_features) &&
    sameEntries(left.field?.prompt_syntaxes, right.field?.prompt_syntaxes) &&
    sameEntries(left.unavailableFeatures, right.unavailableFeatures)
  );
}
/** Propagate dependency reasons to a fixed point in deterministic registry order. */
function dependencies(
  decisions: Map<FeatureId, FeatureDecision>,
  definitions: readonly FeatureDefinition[],
): void {
  let changed = true;
  while (changed) {
    changed = false;
    for (const entry of definitions) {
      if (!decisions.get(entry.id)?.enabled) continue;
      const disabled = entry.dependencies
        .map((id) => decisions.get(id))
        .find((decision) => decision && !decision.enabled);
      if (disabled && !disabled.enabled) {
        decisions.set(entry.id, {
          feature: entry.id,
          enabled: false,
          disabledReason: disabled.disabledReason,
          detail: `Requires ${disabled.feature}.`,
        });
        changed = true;
      }
    }
  }
}
/** Resolve a frozen eligibility profile. Host capabilities describe proven support, not graph objects.
 * The optional registry is an internal pure-policy proof seam for conflict specimens.
 */
export function resolveFeatureProfile(
  preferences: FeaturePreferences,
  context: FeatureContext,
  definitions: readonly FeatureDefinition[] = featureDefinitions,
): FeatureProfile {
  validateFeatureContext(context);
  const allowed = fieldAllows(context);
  const decisions = new Map<FeatureId, FeatureDecision>();
  for (const entry of definitions) {
    const reason = !allowed.has(entry.id)
      ? 'field_disabled'
      : !preferences.allowed[entry.id]
        ? 'user_disabled'
        : null;
    decisions.set(
      entry.id,
      reason
        ? {
            feature: entry.id,
            enabled: false,
            disabledReason: reason,
            detail: null,
          }
        : {
            feature: entry.id,
            enabled: true,
            disabledReason: null,
            detail: null,
          },
    );
  }
  dependencies(decisions, definitions);
  for (const entry of definitions) {
    const schedulingMissing =
      context.mode === 'workflow' &&
      !context.loraScheduling &&
      ['lora_autocomplete', 'lora_picker', 'lora_trigger_words'].includes(entry.id);
    if (
      decisions.get(entry.id)?.enabled &&
      (schedulingMissing || context.unavailableFeatures?.includes(entry.id))
    )
      decisions.set(entry.id, {
        feature: entry.id,
        enabled: false,
        disabledReason: 'missing_service',
        detail: schedulingMissing
          ? 'Prompt Control LoRA scheduling is not available for this prompt field.'
          : 'The host has not supplied this capability.',
      });
  }
  dependencies(decisions, definitions);
  for (const entry of definitions) {
    if (!decisions.get(entry.id)?.enabled) continue;
    for (const conflict of entry.conflicts)
      if (decisions.get(conflict)?.enabled)
        decisions.set(conflict, {
          feature: conflict,
          enabled: false,
          disabledReason: 'conflict',
          detail: `Conflicts with ${entry.id}.`,
        });
  }
  dependencies(decisions, definitions);
  const ordered = definitions.map(({ id }) =>
    Object.freeze(requireValue(decisions.get(id), 'profile')),
  );
  return Object.freeze({
    decisions: Object.freeze(ordered),
    rendererSyntax: Object.freeze(
      definitions
        .filter((entry) => decisions.get(entry.id)?.enabled && entry.syntax !== null)
        .map((entry) => requireValue(entry.syntax, 'profile'))
        .filter((syntax, index, all) => all.indexOf(syntax) === index),
    ),
  });
}
/** All consumers delegate permission checks to the resolved profile. */
export function featureEnabled(profile: FeatureProfile, id: FeatureId): boolean {
  return profile.decisions.some((decision) => decision.feature === id && decision.enabled);
}
