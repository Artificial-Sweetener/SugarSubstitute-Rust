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
/** Define the Vue rail's presentation state without giving it graph or persistence ownership. */
import type { VisualStyle } from '../appearance/VisualStyle.js';
import type { CubeIconModel } from '../../core/CubeIconResolver.js';

/** Share one literal-text row model between live stack navigation and temporary picker drafts. */
export interface CubeStackRowModel {
  readonly id: string;
  readonly label: string;
  readonly secondary: string;
  readonly tooltip: string;
  readonly icon: CubeIconModel;
  readonly targetModel: string;
  readonly bypassed: boolean;
  readonly severity: 'none' | 'warning' | 'error';
  readonly selected: boolean;
}

/** Add canonical graph permissions to a live navigation row. */
export interface CubeStackNavigationRow extends CubeStackRowModel {
  readonly reorderable: boolean;
}

/** Keep visual preference, graph facts and mutation lifetime explicit at the presentation seam. */
export interface CubeStackViewState {
  readonly rows: readonly CubeStackNavigationRow[];
  readonly compact: boolean;
  readonly pending: boolean;
  readonly appearance: VisualStyle;
}

/** Route all user intent to the existing workflow, feedback and preference owners. */
export interface CubeStackViewActions {
  navigate(instanceId: string): void;
  previewOrder(order: readonly string[] | null): void;
  reorder(instanceId: string, order: readonly string[]): Promise<void>;
  move(instanceId: string, delta: -1 | 1): Promise<void>;
  openCart(): void;
  setCompact(compact: boolean): void;
  hide(): void;
  remove(instanceId: string): void;
  contextMenu(instanceId: string, event: MouseEvent): void;
  reportError(error: unknown): void;
}

/** Retain a single Vue application while refreshing its typed state. */
export interface MountedCubeStackView {
  update(state: CubeStackViewState): void;
  setInteractive(interactive: boolean): void;
  dispose(): void;
}
