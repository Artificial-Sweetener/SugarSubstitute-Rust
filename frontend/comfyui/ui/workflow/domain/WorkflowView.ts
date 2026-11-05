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
/** Describe lightweight workflow presentation choices shared by every canvas host. */
export interface WorkflowViewSnapshot {
  readonly canvas: boolean;
  readonly masonry: boolean;
  /** The editor's width beside the output canvas in CSS pixels; zero selects the default width. */
  readonly editorWidth: number;
}

/** Expose accepted workflow view choices independently of rendered surfaces. */
export interface WorkflowViewSource {
  readonly snapshot: WorkflowViewSnapshot;
  readonly ready: boolean;
  subscribe(listener: () => void): () => void;
  change(value: Partial<WorkflowViewSnapshot>): void;
}
