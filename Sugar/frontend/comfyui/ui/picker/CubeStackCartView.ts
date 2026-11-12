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
/** Define immutable picker/cart presentation and intent independently of native mutation. */
import type { CubeStackRowModel, MountedCubeStackView } from '../surface/stack/CubeStackView.js';

/** Describe one captured library artifact and its model/role grouping. */
export interface CubeStackLibraryRow extends CubeStackRowModel {
  readonly catalogKey: string;
  readonly models: readonly string[];
  readonly role: 'Start cubes' | 'Middle cubes' | 'End cubes' | 'Other cubes';
  readonly searchText: string;
}

/** Retain keyed draft rows and accepted appearance across reactive updates. */
export interface CubeStackCartViewState {
  readonly library: readonly CubeStackLibraryRow[];
  readonly rows: readonly CubeStackRowModel[];
  readonly pending: boolean;
  readonly appearance: import('../surface/appearance/VisualStyle.js').VisualStyle;
}

/** Delegate draft intent while the application session remains its sole state owner. */
export interface CubeStackCartViewActions {
  add(catalogKey: string, beforeDraftId?: string | null): void;
  remove(draftId: string): void;
  reorder(draftIds: readonly string[]): void;
}

/** Refresh a single mounted cart without losing search, drag or focus identity. */
export interface MountedCubeStackCartView extends Pick<MountedCubeStackView, 'dispose'> {
  update(state: CubeStackCartViewState): void;
  focusSearch(): void;
}
