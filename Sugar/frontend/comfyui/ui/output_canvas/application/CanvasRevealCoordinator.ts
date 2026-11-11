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
/** Coordinate exact thumbnail requests through the active workflow's canvas destination. */
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasViewId } from '../domain/CanvasIdentity.js';
import { resolveCanvasReveal, type CanvasRevealTarget } from '../domain/CanvasReveal.js';

export interface CanvasRevealDestination {
  /** Prepare and select the exact image using this renderer's existing bounded resource owner. */
  reveal(target: CanvasRevealTarget): Promise<boolean>;
}

export interface CanvasRevealCoordinatorOptions {
  read(): { readonly viewId: CanvasViewId; readonly catalog: CanvasCatalogSnapshot } | null;
  open(view: CanvasViewId): CanvasRevealDestination | null;
}

/** Keep reveal policy renderer-neutral, without retaining files, pixels, or a second output inventory. */
export class CanvasRevealCoordinator {
  /** Bind the active-session authority and an explicit canvas destination. */
  constructor(private readonly options: CanvasRevealCoordinatorOptions) {}

  /** Validate before opening the pane and delegate stale-safe asynchronous preparation. */
  async reveal(target: CanvasRevealTarget): Promise<boolean> {
    const current = this.options.read();
    if (!current || !resolveCanvasReveal(current.catalog, target)) return false;
    const destination = this.options.open(current.viewId);
    const next = this.options.read();
    if (
      !destination ||
      next?.viewId !== current.viewId ||
      !resolveCanvasReveal(next.catalog, target)
    )
      return false;
    return await destination.reveal(target);
  }
}
