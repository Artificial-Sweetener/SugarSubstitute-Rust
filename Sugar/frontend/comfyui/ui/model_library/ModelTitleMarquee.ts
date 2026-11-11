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
/** Resolve Substitute's title holds, constant-speed scroll and glyph-edge fading. */
export interface ModelTitleMarqueeState {
  readonly phase: 'start' | 'scroll' | 'end';
  readonly offset: number;
  readonly leftFade: boolean;
  readonly rightFade: boolean;
}

/** Repeat a readable title cycle independently of browser scheduling and measurement. */
export function modelTitleMarquee(
  elapsedMs: number,
  overflowWidth: number,
): ModelTitleMarqueeState {
  const overflow = Math.max(0, overflowWidth);
  if (overflow === 0) return { phase: 'start', offset: 0, leftFade: false, rightFade: false };
  const duration = Math.max(1, Math.round((overflow / 44) * 1000));
  const position = Math.max(0, elapsedMs) % (900 + duration + 700);
  if (position < 900) return { phase: 'start', offset: 0, leftFade: false, rightFade: true };
  if (position - 900 < duration) {
    const offset = overflow * ((position - 900) / duration);
    return { phase: 'scroll', offset, leftFade: offset > 0.5, rightFade: offset < overflow - 0.5 };
  }
  return { phase: 'end', offset: overflow, leftFade: true, rightFade: false };
}
