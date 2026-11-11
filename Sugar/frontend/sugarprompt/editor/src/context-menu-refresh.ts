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
/** Source-current domain identity admits presentation reuse without carrying old command authority. */
import type { ContextMenu } from './context-menu-state.js';
import type { ContextMenuItem } from './context-menu-contracts.js';

/** The existing local suggestion lane retains its exact measured anchor and indexed finding within one invocation. */
export function sameSpellingMenuRefresh(before: ContextMenu, after: ContextMenu): boolean {
  return (
    before.surface === 'local' &&
    after.surface === 'local' &&
    before.anchor === after.anchor &&
    before.diagnostic?.kind === 'spelling' &&
    after.diagnostic?.kind === 'spelling' &&
    before.diagnostic.finding === after.diagnostic.finding &&
    before.text.freshness === after.text.freshness &&
    before.text.selection.eq(after.text.selection) &&
    before.presets.publication === after.presets.publication &&
    before.utilities === after.utilities
  );
}
/** Stable action identity retains only presentation nodes; activation always receives the newly prepared leaf. */
export function sameRefreshedMenuAction(
  before: ContextMenu,
  after: ContextMenu,
  left: ContextMenuItem,
  right: ContextMenuItem,
): boolean {
  if (left.kind === 'spelling-action' && right.kind === 'spelling-action')
    return left.action === right.action && left.finding === right.finding;
  if (left.kind === 'text' && right.kind === 'text') return left.action === right.action;
  if (left.kind === 'rich-rendering' && right.kind === 'rich-rendering')
    return left.checked === right.checked;
  return (
    left.kind === 'save-segment' &&
    right.kind === 'save-segment' &&
    before.presets.publication === after.presets.publication
  );
}
