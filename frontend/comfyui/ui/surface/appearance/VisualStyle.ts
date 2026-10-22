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
/** Define persisted visual styles independently of browser controls and renderer availability. */
export type VisualStyle = 'comfy' | 'fluent' | 'spectrum' | 'photopro';
export type StyledVisualStyle = Exclude<VisualStyle, 'comfy'>;

/** Accept only styles with an installed presentation implementation. */
export function isVisualStyle(value: unknown): value is VisualStyle {
  return value === 'comfy' || value === 'fluent' || value === 'spectrum' || value === 'photopro';
}

/** Preserve the public Fluent preference until a visual style has been explicitly saved. */
export function resolveVisualStyle(value: unknown, legacyFluent: unknown): VisualStyle {
  return isVisualStyle(value) ? value : legacyFluent === true ? 'fluent' : 'comfy';
}
