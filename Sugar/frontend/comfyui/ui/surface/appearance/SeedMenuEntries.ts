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
/** Describe seed mode choices and restore actions for Fluent and native dropdowns. */
import type { SeedMode } from '../../graph/ComfySeedMode.js';
import type { NativeSeedControl } from './ComfyNativeSeedControl.js';
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import { createSeedActionIcon } from './SeedActionIcons.js';

/** Name native generation modes consistently across both seed menu presentations. */
export const modeLabels: Readonly<Record<SeedMode, string>> = {
  fixed: 'Use Current',
  randomize: 'Randomize',
  increment: 'Increment',
  decrement: 'Decrement',
};

/** Present exactly the supported modes followed by the last-used seed action. */
export function nativeSeedMenuEntries(
  control: NativeSeedControl,
  choose: (mode: SeedMode) => void,
  reuse: () => void,
  fluent = true,
): readonly ContextMenuEntry[] {
  return [
    ...control.modes.map(
      (mode): ContextMenuEntry => ({
        label: modeLabels[mode],
        checked: control.readMode() === mode,
        attributes: { 'data-sc-seed-option': mode },
        icon: (document) => createSeedActionIcon(document, mode, fluent),
        select: () => choose(mode),
      }),
    ),
    {
      label: 'Reuse last',
      disabled: !control.reuse || control.reuse.readLast() === null,
      attributes: { 'data-sc-seed-option': 'reuse-last' },
      icon: (document) => createSeedActionIcon(document, 'reuse-last', fluent),
      select: reuse,
    },
  ];
}
