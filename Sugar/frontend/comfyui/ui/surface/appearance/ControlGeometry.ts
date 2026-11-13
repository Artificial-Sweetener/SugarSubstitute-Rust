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
/** Share field geometry across control implementations. */
export const CONTROL_HEIGHT = 'var(--sc-control-height,33px)';

/** Reserve five digit cells for four-digit values, signs or decimals, plus field chrome. */
export const SPINNER_WIDTH = 'calc(5ch + 18px + var(--sc-spinner-stepper-width,20px))';

/** Retain the established slider width beside native number spinners. */
export const NUMERIC_SLIDER_WIDTH = '120px';

/** Share spacing and total width for the existing slider-and-spinner composition. */
export const NUMERIC_PAIR_GAP = '6px';

/** Derive pair width from its shared slider, spinner, and spacing dimensions. */
export const NUMERIC_PAIR_WIDTH = `calc(${SPINNER_WIDTH} + ${NUMERIC_SLIDER_WIDTH} + ${NUMERIC_PAIR_GAP})`;
