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
/** Retain navigation controls during continuous camera/wipe status updates. */

import type { CanvasResourceStatus, CanvasViewSnapshot } from '../../domain/CanvasViewPort.js';
import { sameCanvasNavigation } from '../../domain/CanvasViewState.js';

/** Keep catalog-bearing component props stable until a navigation-relevant value changes. */
export function canvasNavigationSnapshot(
  current: CanvasViewSnapshot,
  previous: CanvasViewSnapshot | undefined,
): CanvasViewSnapshot {
  return previous &&
    previous.sources === current.sources &&
    previous.scenes === current.scenes &&
    sameCanvasNavigation(previous.view, current.view) &&
    previous.view.comparison?.orientation === current.view.comparison?.orientation &&
    previous.selected?.id === current.selected?.id &&
    previous.secondary?.id === current.secondary?.id &&
    previous.preview === current.preview &&
    sameResource(previous.resource, current.resource) &&
    sameResource(previous.secondaryResource, current.secondaryResource)
    ? previous
    : current;
}

/** Compare product feedback without reacting to fresh immutable wrapper identities. */
function sameResource(
  previous: CanvasResourceStatus | null,
  current: CanvasResourceStatus | null,
): boolean {
  return (
    previous?.state === current?.state &&
    previous?.message === current?.message &&
    previous?.canRetry === current?.canRetry &&
    previous?.canAdmitLargeImage === current?.canAdmitLargeImage
  );
}
