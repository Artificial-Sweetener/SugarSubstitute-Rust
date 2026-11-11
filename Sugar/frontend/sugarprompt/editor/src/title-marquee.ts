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
/** Scalar animation phases consume prepared overflow, without reading DOM or owning a clock. */
import { roundToEven } from './round-to-even.js';

/** The current title window holds readable endpoints and fades only moving glyphs. */
export interface TitleMarqueeState {
  readonly phase: 'start' | 'scroll' | 'end';
  readonly offset: number;
  readonly showLeftFade: boolean;
  readonly showRightFade: boolean;
}

/** Resolve the adopted 900ms/44px-per-second/700ms title cycle from monotonic elapsed milliseconds and CSS-local overflow. */
export function titleMarqueeState(elapsedMs: number, overflow: number): TitleMarqueeState {
  if (overflow <= 0)
    return {
      phase: 'start',
      offset: 0,
      showLeftFade: false,
      showRightFade: false,
    };
  const duration = Math.max(1, roundToEven((overflow / 44) * 1000)),
    position = elapsedMs % (900 + duration + 700);
  if (position < 900)
    return {
      phase: 'start',
      offset: 0,
      showLeftFade: false,
      showRightFade: true,
    };
  if (position < 900 + duration) {
    const offset = overflow * ((position - 900) / duration);
    return {
      phase: 'scroll',
      offset,
      showLeftFade: offset > 0.5,
      showRightFade: offset < overflow - 0.5,
    };
  }
  return {
    phase: 'end',
    offset: overflow,
    showLeftFade: true,
    showRightFade: false,
  };
}
