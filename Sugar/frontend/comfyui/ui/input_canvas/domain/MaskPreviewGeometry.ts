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
/** Describe edited-mask preview framing independently of display controls and native editors. */
import type { CanvasImageRectangle } from '../../../../../integrations/contracts/CanvasImageRegion.js';
/** Select canvas context or the exact prepared input without changing processing intent. */
export type MaskPreviewMode = 'context' | 'input';

/** Locate processing coverage inside a full source frame for context-mode washing. */
export interface MaskPreviewPlacement {
  readonly bounds: CanvasImageRectangle;
  readonly frame: { readonly width: number; readonly height: number };
}
