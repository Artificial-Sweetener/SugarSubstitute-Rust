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
/** Let a model wall tell the model library which models a person is looking at. */
import type { InjectionKey } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';

/**
 * Provided by the application that mounts a wall, when it can reach the model library. The wall
 * reports the models on screen, so their names and images are looked up before the rest.
 */
export const shownModels: InjectionKey<(items: readonly ModelCatalogItem[]) => void> =
  Symbol('sugarcubes shown models');
