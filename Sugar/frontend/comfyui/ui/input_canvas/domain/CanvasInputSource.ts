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
/** Define source choices independently of the feature consuming captured canvas pixels. */
import type {
  CanvasInputSource,
  CanvasInputTarget,
  CanvasSourceContext,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';

/** Preserve one authoritative ordering and labeling for reusable source controls. */
export const CANVAS_INPUT_CHOICES: readonly {
  readonly kind: CanvasInputTarget['kind'];
  readonly label: string;
}[] = [
  { kind: 'selection', label: 'Canvas Selection' },
  { kind: 'current-layer', label: 'Current Layer' },
  { kind: 'layer', label: 'Specific Layer' },
  { kind: 'composition', label: 'Current Composition' },
  { kind: 'composition-from-layer', label: 'Current Comp From Layer' },
  { kind: 'file', label: 'Load From File' },
];

/** Bind explicit layers to their document while leaving current targets responsive to active context. */
export function chooseCanvasInput(
  canvasId: string,
  context: CanvasSourceContext | null,
  target: CanvasInputTarget,
): CanvasInputSource {
  if (target.kind === 'file') return target;
  if (!canvasId || !context?.supported || !context.targets.includes(target.kind))
    throw new Error('The connected canvas does not support this source.');
  const explicit = target.kind === 'layer' || target.kind === 'composition-from-layer';
  if (
    explicit &&
    (!context.document || !context.layers.some((layer) => layer.id === target.layerId))
  )
    throw new Error('Choose an available layer in the connected canvas.');
  return {
    kind: 'canvas',
    canvasId,
    documentId: explicit ? (context.document?.id ?? null) : null,
    target,
  };
}

/** Prefer current selection only when that operation is advertised by the connected canvas. */
export function defaultCanvasInput(
  canvasId: string,
  context: CanvasSourceContext | null,
  file: string | null,
  preferred: 'selection' | 'composition',
): CanvasInputSource {
  return context?.supported && context.targets.includes(preferred)
    ? chooseCanvasInput(canvasId, context, { kind: preferred })
    : { kind: 'file', file };
}
