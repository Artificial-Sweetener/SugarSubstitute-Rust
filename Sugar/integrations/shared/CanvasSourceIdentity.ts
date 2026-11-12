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
/** Compare portable source identities independently of JSON object property ordering. */
import type {
  CanvasDocument,
  CanvasSourceTarget,
  CanvasInputSource,
} from '../contracts/CanvasProtocol.generated.js';

/** Key the selected operation and explicit layer identity without including consumer settings. */
export function canvasSourceTargetKey(target: CanvasSourceTarget): string {
  return JSON.stringify([
    target.kind,
    target.kind === 'layer' || target.kind === 'composition-from-layer' ? target.layerId : null,
  ]);
}

/** Distinguish live active targets from explicit document-bound layers and authored files. */
export function canvasInputSourceKey(source: CanvasInputSource): string {
  return source.kind === 'file'
    ? JSON.stringify(['file', source.file])
    : JSON.stringify([
        'canvas',
        source.canvasId,
        source.documentId,
        canvasSourceTargetKey(source.target),
      ]);
}

/** Keep pixel geometry and interpretation qualified by the exact adapter document revision. */
export function sameCanvasSourceDocument(first: CanvasDocument, second: CanvasDocument): boolean {
  return (
    first.id === second.id &&
    first.geometryRevision === second.geometryRevision &&
    first.width === second.width &&
    first.height === second.height
  );
}
