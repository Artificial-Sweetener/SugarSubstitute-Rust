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
/** Project accepted proximity prompt links for Cube media ancestry without taking execution policy ownership. */
import type { ProximityMatch } from '../overlays/proximity/ProximityModel.js';
import type { CubePreviewLink } from './ComfyCubePreviewCatalog.js';

/** Preserve the existing preview boundary by excluding disabled proximity and incomplete endpoints. */
export function projectCubeProximityPreviewLinks(
  enabled: boolean,
  matches: readonly Pick<ProximityMatch, 'outputId' | 'outputSlot' | 'inputId'>[],
): readonly CubePreviewLink[] {
  return enabled
    ? matches.flatMap((match) =>
        match.outputId === undefined || match.inputId === undefined
          ? []
          : [{ originId: match.outputId, originSlot: match.outputSlot, targetId: match.inputId }],
      )
    : [];
}
