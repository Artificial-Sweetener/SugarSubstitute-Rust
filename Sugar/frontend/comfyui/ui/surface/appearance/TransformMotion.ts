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
/** Own Substitute's spatial transform motion: Qt InOutCubic over 220 ms, reproduced exactly. */

/** Duration of Substitute's cube-stack density transform, shared by everything that travels with it. */
export const transformMotion = { duration: 220 } as const;

/** Select the curve for reserved-space travel: the shared surface curve or Substitute's transform. */
export type InsetMotion = 'surface' | 'transform';

/**
 * Build Qt InOutCubic keyframes from one linear frame interpolation.
 *
 * Each half has linear x and cubic y, so the two Bezier segments equal InOutCubic exactly rather
 * than approximating it with one curve.
 */
export function inOutCubicKeyframes(frame: (progress: number) => Keyframe): Keyframe[] {
  return [
    {
      ...frame(0),
      offset: 0,
      easing: 'cubic-bezier(0.3333333333333333,0,0.6666666666666666,0)',
    },
    {
      ...frame(0.5),
      offset: 0.5,
      easing: 'cubic-bezier(0.3333333333333333,1,0.6666666666666666,1)',
    },
    { ...frame(1), offset: 1 },
  ];
}
