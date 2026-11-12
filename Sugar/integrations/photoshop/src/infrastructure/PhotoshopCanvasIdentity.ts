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
/** Resolve explicit document identities and geometry for every Photoshop canvas capability. */
import { app } from 'photoshop';
import type { CanvasDocument } from '../../../contracts/CanvasProtocol.generated.js';
export type PhotoshopDocument = NonNullable<typeof app.activeDocument>;
/** Refuse to substitute a later active document for a captured identity. */
export function findCanvasDocument(id: string): PhotoshopDocument | null {
  return app.documents.find((document) => String(document.id) === id) ?? null;
}
/** Qualify pixel coordinates by geometry and color interpretation rather than unrelated content edits. */
export function canvasDocumentIdentity(document: PhotoshopDocument): CanvasDocument {
  return {
    id: String(document.id),
    geometryRevision: JSON.stringify([
      document.width,
      document.height,
      document.mode,
      document.bitsPerChannel,
      document.colorProfileName,
      document.pixelAspectRatio,
    ]),
    width: document.width,
    height: document.height,
  };
}
