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
/**
 * The slider the canvas tool options draw. The canvas attachment that mounts the editing chrome
 * supplies it, so the options show the same slider as the node cards without this layer
 * depending on the node-card appearance.
 */
import type { InjectionKey } from 'vue';

/** One mounted option slider. */
export interface CanvasOptionSlider {
  /** The slider element; it may be replaced where it stands when the appearance changes. */
  readonly root: HTMLElement;
  synchronize(
    minimum: number,
    maximum: number,
    step: number,
    disabled: boolean,
    value: number | null,
  ): void;
  dispose(): void;
}

/** What one option's slider presents. */
export interface CanvasOptionSliderSpec {
  readonly name: string;
  readonly label: string;
  readonly minimum: number;
  readonly maximum: number;
  readonly step: number;
  /** Decimal places the value shows. */
  readonly precision: number;
  /** Attributes the slider element carries, including after a replacement. */
  readonly attributes: Readonly<Record<string, string>>;
}

/** Create one option slider; `input` receives each value a drag passes. */
export type CanvasOptionSliders = (
  document: Document,
  option: CanvasOptionSliderSpec,
  input: (value: number) => void,
) => CanvasOptionSlider;

/** The editing chrome's slider factory, provided where the overlay mounts. */
export const CANVAS_OPTION_SLIDERS: InjectionKey<CanvasOptionSliders> =
  Symbol('canvas option sliders');
