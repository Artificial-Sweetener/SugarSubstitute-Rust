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
/** Define the native widget mount boundary consumed by toolbar presentation. */
import type { FieldValue, OverrideGroup, RelationSeedMode } from '../domain/RelationContracts.js';

export interface OverrideWidgetMount {
  update(group: OverrideGroup): void;
  dispose(): void;
}

export interface RelationWidgetChoice {
  value: string;
  label: string;
  blockers: readonly string[];
}

export interface RelationChoiceMount {
  update(
    value: string,
    choices: readonly RelationWidgetChoice[],
    disabled?: boolean,
    preferredLabels?: readonly string[],
  ): void;
  dispose(): void;
}

/** Render a shared value through the host's standard widget component family. */
export interface RelationWidgetRenderer {
  mount(
    root: HTMLElement,
    group: OverrideGroup,
    change: (value: FieldValue) => void,
    changeMode: (mode: RelationSeedMode) => void,
  ): OverrideWidgetMount;
  mountChoice(
    root: HTMLElement,
    label: string,
    change: (value: string) => void,
    nativeInputName?: string,
  ): RelationChoiceMount;
}
