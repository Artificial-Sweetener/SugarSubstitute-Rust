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
/** Declare detector presentation and preference boundaries independently from Vue and HTTP. */
import type { ModelCatalogItem } from './ModelCatalog.js';

export interface ModelDetectorPort {
  resolve(values: readonly string[], revision: number): Promise<readonly ModelCatalogItem[]>;
  library(signal: AbortSignal): Promise<readonly ModelCatalogItem[]>;
  assign(value: string, asset: string, signal: AbortSignal): Promise<void>;
}
