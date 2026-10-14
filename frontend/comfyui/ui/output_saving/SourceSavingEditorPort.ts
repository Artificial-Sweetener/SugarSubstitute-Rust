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
/** Describe a fenced workflow edit without exposing native graph objects to presentation. */
import type { SourceSavingPreference } from './domain/SourceSavingPreferences.js';

export interface SavingSourceTarget {
  readonly path: string;
  readonly title: string;
  readonly classType: string;
  readonly supported: boolean;
  readonly kind: 'save' | 'preview' | null;
  readonly preference: SourceSavingPreference;
}
export interface SourceSavingEditSession {
  readonly targets: readonly SavingSourceTarget[];
  /** Reject replaced workflow or source handles before presenting or committing an edit. */
  current(): boolean;
  /** Commit one coherent history transaction or reject a replaced workflow/source. */
  commit(values: ReadonlyMap<string, SourceSavingPreference>): void;
}
export interface SourceSavingEditorPort {
  /** Capture the active workflow and cached source handles for this explicit edit. */
  begin(): SourceSavingEditSession;
}
