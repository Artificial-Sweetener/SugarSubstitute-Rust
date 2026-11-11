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
/** Record native path-builder commands without reproducing Comfy's routing formulas. */
export interface NativeLinkPathStroke {
  link: string;
  path: string;
  transform: string;
  color: string;
  width: number;
  opacity: number;
  lineCap: CanvasLineCap;
}

export interface NativeLinkPathSnapshot {
  strokes: readonly NativeLinkPathStroke[];
  restore(): void;
}

/** Accept the same drawing commands as native paths and retain only their vector representation. */
export class NativeLinkPathRecording {
  readonly #commands: string[] = [];

  /** Begin a native segment at its output socket. */
  moveTo(x: number, y: number): void {
    this.#record('M', [x, y]);
  }

  /** Preserve native straight or orthogonal segments. */
  lineTo(x: number, y: number): void {
    this.#record('L', [x, y]);
  }

  /** Preserve both native spline controls and its input endpoint. */
  bezierCurveTo(a: number, b: number, c: number, d: number, x: number, y: number): void {
    this.#record('C', [a, b, c, d, x, y]);
  }

  /** Preserve native single-control curves. */
  quadraticCurveTo(a: number, b: number, x: number, y: number): void {
    this.#record('Q', [a, b, x, y]);
  }

  /** Expose validated vector commands after the native builder finishes. */
  get path(): string {
    return this.#commands.join(' ');
  }

  /** Reject invalid external coordinates before they reach the SVG presentation. */
  #record(command: string, coordinates: readonly number[]): void {
    if (!coordinates.every(Number.isFinite))
      throw new Error('Native link path contains invalid coordinates.');
    this.#commands.push(`${command}${coordinates.join(' ')}`);
  }
}
