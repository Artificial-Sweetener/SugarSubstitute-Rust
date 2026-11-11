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
/** Describe identified native controls without exposing graph traversal to search presentation. */
import type { SearchNode } from '../domain/SearchMatching.js';
export type SearchTextControl = HTMLInputElement | HTMLTextAreaElement;
export interface SearchFieldSurface {
  readonly id: string;
  readonly node: string;
  readonly card: HTMLElement;
  readonly row: HTMLElement;
  readonly control: SearchTextControl | null;
}
/** Read values from a lifecycle-owned corpus while keeping mounted lookup stable during edits. */
export interface SearchSurface {
  readonly fields: ReadonlyMap<string, SearchFieldSurface>;
  read(): readonly SearchNode[];
}
