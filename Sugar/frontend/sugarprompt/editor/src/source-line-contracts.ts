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
/** Source-line presentation contracts expose native geometry without a second text/layout owner. */

/** Host intent reserves extra CSS-local space for line numbers and enables row backgrounds. */
export interface SourceLineChromeOptions {
  readonly enabled: boolean;
  readonly contentLeftInset: number;
}

/** A zero-based source line spans wrapped rows; scroller-local coordinates normalize through the snapshot's native scale. */
export interface SourceLineRect {
  readonly lineIndex: number;
  readonly top: number;
  readonly left: number;
  readonly width: number;
  readonly height: number;
}

/** Prepared visible rectangles remain unavailable during invalidation and unfinished native composition. */
export interface SourceLineSnapshot {
  readonly status: 'pending' | 'ready' | 'composing';
  readonly revision: number;
  readonly currentLineIndex: number;
  readonly focusActive: boolean;
  readonly chrome: SourceLineChromeOptions;
  readonly viewportWidth: number;
  readonly viewportHeight: number;
  readonly scaleX: number;
  readonly scaleY: number;
  readonly lines: readonly SourceLineRect[];
}

/** Reject malformed visual intent before native style or configuration publication. */
export function validateSourceLineChrome(options: SourceLineChromeOptions): void {
  if (
    !options ||
    typeof options.enabled !== 'boolean' ||
    !Number.isFinite(options.contentLeftInset) ||
    options.contentLeftInset < 0 ||
    options.contentLeftInset > 100_000
  )
    throw new RangeError('Invalid source-line chrome configuration');
}
