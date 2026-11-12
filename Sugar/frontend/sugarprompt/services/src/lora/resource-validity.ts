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
/** Catalog authority, independently of source coverage, determines whether a resolved descriptor remains reusable. */
import type { LoraResource } from './contracts.js';

/** Found, missing and ambiguous outcomes belong to the immutable catalog; unavailable and pending authority require another request. */
export function isCatalogStableLoraResource(resource: LoraResource): boolean {
  return (
    resource.resolution === 'found' ||
    resource.resolution === 'missing' ||
    resource.resolution === 'ambiguous'
  );
}
