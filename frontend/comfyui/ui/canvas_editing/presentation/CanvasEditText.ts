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
/** Hold every user-facing phrase of the canvas editing chrome in one catalogue. */
import type { CanvasToolId } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasCoverageOperation } from '../domain/CanvasCoverageModification.js';

/** Tool names for rail tooltips, flyout rows and session labels. */
export const CANVAS_TOOL_LABELS: Readonly<Record<CanvasToolId, string>> = Object.freeze({
  'pan-zoom': 'Pan & Zoom',
  move: 'Move',
  brush: 'Brush',
  eraser: 'Eraser',
  'select-rectangle': 'Rectangle selection',
  'select-ellipse': 'Ellipse selection',
  'select-lasso': 'Lasso selection',
  'mask-rectangle': 'Rectangle Mask',
  'mask-ellipse': 'Ellipse Mask',
  'mask-lasso': 'Lasso Mask',
  transform: 'Transform',
  'shared-edges': 'Resize shared edges',
});

/** Coverage operation names for the operation selector. */
export const CANVAS_COVERAGE_OPERATION_LABELS: Readonly<Record<CanvasCoverageOperation, string>> =
  Object.freeze({ expand: 'Expand', contract: 'Contract', feather: 'Feather' });

/** Every other phrase of the editing chrome. */
export const CANVAS_EDIT_TEXT = Object.freeze({
  tools: 'Canvas tools',
  toolGroup: (label: string) => `${label} tools`,
  brushSettings: 'Brush settings',
  size: 'Size',
  hardness: 'Hardness',
  opacity: 'Opacity',
  pixelValue: (value: number) => `${value} px`,
  percentValue: (value: number) => `${value}%`,
  close: 'Close',
  undo: 'Undo',
  redo: 'Redo',
  history: 'History',
  apply: 'Apply',
  cancel: 'Cancel',
  modifySelection: 'Modify selection',
  selectionActions: 'Selection',
  transform: 'Transform',
  transformSelection: 'Transform selection',
  clear: 'Clear',
  deselect: 'Deselect',
  rotateLeft: 'Rotate left',
  rotateRight: 'Rotate right',
  flipHorizontal: 'Flip horizontal',
  flipVertical: 'Flip vertical',
  operation: 'Operation',
  by: 'by',
  pixels: 'Pixels',
  editLayerCoverage: 'Edit layer coverage',
  canvasActions: 'Canvas actions',
});
