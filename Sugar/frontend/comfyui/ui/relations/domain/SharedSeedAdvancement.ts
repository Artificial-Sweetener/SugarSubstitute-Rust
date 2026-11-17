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
/** Advance one seed per effective shared group using its common numeric domain. */
import type {
  FieldContract,
  FieldDomain,
  FieldValue,
  NodeContract,
  OverrideGroup,
  RelationSeedMode,
  RelationState,
} from './RelationContracts.js';
import { commonFieldDomain } from './FieldCompatibility.js';
import { linkedFieldMembers, resolveRelations } from './RelationResolver.js';

/** The host's own rule for the value a controlled number takes next. */
export type SeedStep = (
  domain: Extract<FieldDomain, { kind: 'number' }>,
  current: number,
  mode: Exclude<RelationSeedMode, 'fixed'>,
) => number;

/** Update globals and linked masters once while leaving native independent seeds to Comfy. */
export function advanceSharedSeeds(
  fields: readonly FieldContract[],
  nodes: readonly NodeContract[],
  groups: readonly OverrideGroup[],
  state: RelationState,
  step: SeedStep,
): void {
  for (const group of groups) {
    if (!group.enabled || group.members[0]?.semantic !== 'seed') continue;
    const override = state.overrides[group.key];
    if (override) override.value = advanceSeed(group.domain, group.value, group.seedMode, step);
  }
  const resolved = resolveRelations(fields, nodes, state, groups);
  for (const item of resolved) {
    if (item.field.semantic !== 'seed' || item.override || item.source) continue;
    const members = linkedFieldMembers(item.field.id, fields, nodes, state);
    if (members.length < 2) continue;
    const mode = state.localSeedModes[item.field.id] ?? item.field.seedMode ?? 'fixed';
    state.locals[item.field.id] = advanceSeed(commonFieldDomain(members), item.value, mode, step);
  }
}

/**
 * Give one shared seed its next value by the host's rule, so a shared seed and an unshared one
 * are rolled and stepped identically.
 *
 * A seed beyond a page number's exact range was not rolled here: it came from an image or was
 * typed. One step from it cannot be held, so stepping leaves it as it is.
 */
function advanceSeed(
  domain: FieldDomain | null,
  current: FieldValue,
  mode: RelationSeedMode,
  step: SeedStep,
): FieldValue {
  if (domain?.kind !== 'number' || typeof current !== 'number' || mode === 'fixed') return current;
  if (mode !== 'randomize' && !Number.isSafeInteger(current)) return current;
  return step(domain, current, mode);
}
