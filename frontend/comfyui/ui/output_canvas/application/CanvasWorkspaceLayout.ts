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
/** Own transient resize and drawer intent while persistence remains with the host preference adapter. */

import {
  allocateCanvasWorkspace,
  type CanvasWorkspacePreferences,
} from '../domain/CanvasWorkspace.js';

/** Publish one allocation policy to browser regions without storing results or workflow state. */
export class CanvasWorkspaceLayout {
  #preferences: CanvasWorkspacePreferences = Object.freeze({ visible: true, width: 0 });
  #width = 0;
  #height = 0;
  #drawer = false;
  #draft: number | null = null;

  /** Accept only committed preferences and relinquish their superseded width preview. */
  preferences(preferences: CanvasWorkspacePreferences): void {
    allocateCanvasWorkspace(this.#width, this.#height, preferences, this.#drawer);
    this.#preferences = Object.freeze({ ...preferences });
    this.#draft = null;
    if (!preferences.visible) this.#drawer = false;
  }

  /** Collapse a drawer on breakpoint entry instead of opening it as a resize side effect. */
  bounds(width: number, height: number): void {
    allocateCanvasWorkspace(width, height, this.#preferences, this.#drawer);
    if (width < 960 && this.#width >= 960) this.#drawer = false;
    this.#width = width;
    this.#height = height;
  }

  /** Apply an explicitly accepted drawer action without changing the user's desktop width. */
  drawer(open: boolean): void {
    this.#drawer = open;
  }

  /** Preview a local resize through the same clamps used for settled allocations. */
  resize(width: number): number {
    if (!Number.isFinite(width) || width < 0)
      throw new RangeError('Canvas resize width is invalid.');
    const allocation = allocateCanvasWorkspace(
      this.#width,
      this.#height,
      { ...this.#preferences, width: Math.max(1, width) },
      this.#drawer,
    );
    this.#draft = allocation.canvas.width;
    return this.#draft;
  }

  /** Restore the accepted width when a captured resize is canceled or its attachment retires. */
  cancelResize(): void {
    this.#draft = null;
  }

  /** Read immutable rectangles for all consumers, including a zero-width hidden canvas. */
  read() {
    return allocateCanvasWorkspace(
      this.#width,
      this.#height,
      { ...this.#preferences, width: this.#draft ?? this.#preferences.width },
      this.#drawer,
    );
  }
}
