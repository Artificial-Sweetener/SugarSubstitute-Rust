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
/** Describe the ComfyUI element helper consumed by the extension. */

export type ElementAttributes = Record<string, unknown>;
export type ElementChild = Node | string | null | undefined;
type ElementForSelector<Selector extends string> =
  Selector extends `${infer Tag}.${string}`
    ? Tag extends keyof HTMLElementTagNameMap
      ? HTMLElementTagNameMap[Tag]
      : HTMLElement
    : Selector extends keyof HTMLElementTagNameMap
      ? HTMLElementTagNameMap[Selector]
      : HTMLElement;

export function $el<Selector extends string>(
  tag: Selector,
  children?: ElementChild[],
): ElementForSelector<Selector>;
export function $el<Selector extends string>(
  tag: Selector,
  textContent: string,
): ElementForSelector<Selector>;
export function $el<Selector extends string>(
  tag: Selector,
  attributes?: ElementAttributes,
  children?: ElementChild[],
): ElementForSelector<Selector>;
