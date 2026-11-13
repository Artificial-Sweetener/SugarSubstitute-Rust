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
/** Adapt Microsoft's horizontal slider coordinates to transformed Comfy graph surfaces. */
import type { Slider } from '@fluentui/web-components/slider/class.js';

/** Normalize screen distances while retaining Fluent's value, stepping, and drag owners. */
export function adaptFluentSliderGeometry(slider: Slider): void {
  const calculate = slider.calculateNewValue.bind(slider);
  slider.calculateNewValue = (screenDistance: number): number => {
    const track = slider.track;
    const screenWidth = track?.getBoundingClientRect().width ?? 0;
    const layoutWidth = track?.clientWidth ?? 0;
    return calculate(
      screenWidth > 0 && layoutWidth > 0
        ? (screenDistance * layoutWidth) / screenWidth
        : screenDistance,
    );
  };
}
