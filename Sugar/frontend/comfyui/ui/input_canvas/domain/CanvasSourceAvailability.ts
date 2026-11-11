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
/** Classify expected target unavailability independently of preview and execution consumers. */
import type {
  CanvasInputSource,
  CanvasSourceContext,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';

/** Preserve source identity so one unavailable target does not invalidate other previews. */
export class CanvasSourceUnavailable extends Error {
  /** Keep actionable, concise feedback separate from unexpected capture faults. */
  constructor(
    message: string,
    readonly source: CanvasInputSource | null = null,
  ) {
    super(message);
    this.name = 'CanvasSourceUnavailable';
  }
}

/** Explain missing source prerequisites without requesting native pixels or changing authored intent. */
export function canvasSourceUnavailable(
  source: CanvasInputSource,
  context: CanvasSourceContext | null,
): string | null {
  if (source.kind === 'file') return source.file ? null : 'Choose a file';
  if (!context?.supported) return 'Canvas unavailable';
  if (!context.document) return 'Open a document';
  if (source.documentId !== null && source.documentId !== context.document.id)
    return 'Open the source document';
  if (!context.targets.includes(source.target.kind)) return 'Source unavailable';
  const target = source.target;
  if (
    target.kind === 'current-layer' &&
    (!context.currentLayerId ||
      !context.layers.some((layer) => layer.id === context.currentLayerId))
  )
    return 'Select a layer';
  if (
    (target.kind === 'layer' || target.kind === 'composition-from-layer') &&
    !context.layers.some((layer) => layer.id === target.layerId)
  )
    return 'Choose a layer';
  return null;
}
