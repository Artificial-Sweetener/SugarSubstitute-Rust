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
/** Name the only two ways a native attachment's preparation can end without becoming ready. */

/**
 * A surface whose host reported that it cannot present, or whose transport to the host failed.
 *
 * This is an observed failure, never a verdict reached because the host took long: a host that is
 * merely slow leaves its attachment loading, with no outcome at all.
 */
export class CanvasPreparationUnavailable extends Error {
  /** Preserve the native owner's actionable failure message. */
  constructor(message: string) {
    super(message);
    this.name = 'CanvasPreparationUnavailable';
  }
}

/**
 * An attachment that was released (for example by a workflow switch) before its host answered.
 *
 * It carries no user-visible failure: the host's late answer is simply no longer wanted.
 */
export class CanvasPreparationReleased extends Error {
  /** Name the cause for diagnostics. */
  constructor() {
    super('Native canvas attachment was released before preparation completed.');
    this.name = 'CanvasPreparationReleased';
  }
}
