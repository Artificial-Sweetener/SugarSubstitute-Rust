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
/** Mounted diagnostics retain immutable ignored identities across source/profile and host baseline replacement. */
import { requireValue } from '../../core/src/required.js';
import { StateEffect, StateField, Transaction, type EditorState } from '@codemirror/state';
/** One diagnostic identity uses its exact storage UTF-16 duplicate range and canonical key, matching the donor session contract. */
export function duplicateIgnoreKey(from: number, to: number, key: string): string {
  return JSON.stringify([from, to, key]);
}
/** Only derived diagnostics change; ignoring never changes source, selection or native history. */
export const ignoreDuplicate = StateEffect.define<string>();
const carry = StateEffect.define<readonly string[]>();
const membership = new WeakMap<readonly string[], ReadonlySet<string>>();
/** Memoize membership under the complete immutable authoritative field identity; no independent session state is retained. */
function indexed(keys: readonly string[]): readonly string[] {
  const value = Object.freeze([...keys]);
  membership.set(value, new Set(value));
  return value;
}
/** Native hit testing observes one prepared membership index without traversing the mounted session's ignored identities. */
export function duplicateIgnored(state: EditorState, key: string): boolean {
  const value = state.field(duplicateIgnores);
  return requireValue(membership.get(value), 'duplicate-ignore').has(key);
}
/** A fresh mounted state starts empty; source/context transactions deliberately retain the session's immutable identity set. */
export const duplicateIgnores = StateField.define<readonly string[]>({
  create: () => indexed([]),
  update(value, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(carry)) value = effect.value;
      if (
        effect.is(ignoreDuplicate) &&
        !requireValue(membership.get(value), 'duplicate-ignore').has(effect.value)
      )
        value = indexed([...value, effect.value]);
    }
    return value;
  },
});
/** The mounted surface carries this actual field through a host baseline; headless or unconfigured states retain their original baseline contract. */
export function carryDuplicateIgnores(from: EditorState, to: EditorState): EditorState {
  const value = from.field(duplicateIgnores, false);
  if (!value || !to.field(duplicateIgnores, false) || value.length === 0) return to;
  return to.update({
    effects: carry.of(value),
    annotations: Transaction.addToHistory.of(false),
  }).state;
}
/** A newly mounted surface starts a new diagnostics session even when its unbound baseline previously carried another session's identities. */
export function beginDuplicateIgnoreSession(state: EditorState): EditorState {
  if (!state.field(duplicateIgnores, false)?.length) return state;
  return state.update({
    effects: carry.of(indexed([])),
    annotations: Transaction.addToHistory.of(false),
  }).state;
}
