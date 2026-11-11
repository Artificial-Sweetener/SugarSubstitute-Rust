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
/** Present seed actions using the active Fluent or native Comfy icon family. */
import type { SeedMode } from '../../graph/ComfySeedMode.js';

type SeedAction = SeedMode | 'reuse-last';
const fluentIcons: Readonly<Record<SeedAction, string>> = {
  fixed: 'locked',
  randomize: 'game-die',
  increment: 'add',
  decrement: 'subtract',
  'reuse-last': 'reuse-last',
};
const nativeIcons = {
  fixed: 'pencil-off',
  randomize: 'shuffle',
  'reuse-last': 'rotate-ccw',
} as const;

/** Resolve the pinned Microsoft asset used by Fluent seed controls. */
export function seedIconImage(action: SeedAction): string {
  const url = new URL(`../../../../icons/seed/${fluentIcons[action]}.svg`, import.meta.url);
  return `url("${url.href}")`;
}

/** Keep icon colors inherited while matching Comfy's native +1/-1 generation glyphs. */
export function createSeedActionIcon(
  document: Document,
  action: SeedAction,
  fluent: boolean,
): HTMLElement {
  const icon = document.createElement('span');
  icon.dataset.scSeedIcon = '';
  icon.dataset.scSeedIconFamily = fluent ? 'fluent' : 'comfy';
  icon.style.cssText =
    'width:16px;height:16px;display:inline-flex;align-items:center;justify-content:center;flex:none;';
  if (fluent) {
    icon.style.background = 'currentColor';
    icon.style.mask = `${seedIconImage(action)} center/contain no-repeat`;
    icon.style.setProperty('--sc-seed-icon', seedIconImage(action));
  } else if (action === 'increment' || action === 'decrement') {
    icon.textContent = action === 'increment' ? '+1' : '-1';
  } else icon.className = `icon-[lucide--${nativeIcons[action]}]`;
  return icon;
}
