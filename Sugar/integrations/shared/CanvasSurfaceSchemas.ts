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
 * Own the surface discovery schemas this release knows and how declarations step down between them.
 *
 * Either side of a session may be newer than the other. A host answers a request for a schema it
 * does not know with the newest schema it knows, and a page accepts an older answer by keeping
 * only what that schema can express. Capabilities a schema cannot express are unsupported.
 */
import type { CanvasSurfaceCapabilities } from '../contracts/CanvasSurface.js';

/** Request and answer with the newest feature schema this release implements. */
export const NEWEST_CANVAS_SURFACE_SCHEMA = 3;

/** Schema one predates the native surface descriptor and can express no executable surface. */
export const OLDEST_CANVAS_SURFACE_SCHEMA = 2;

/** Bound the schema a discovery answers with by the request, this release and the host's declaration. */
export function answeredCanvasSurfaceSchema(requested: number, declared: number): number {
  return Math.min(requested, declared, NEWEST_CANVAS_SURFACE_SCHEMA);
}

/** Report whether a schema can express any executable surface operation. */
export function expressesCanvasSurface(schema: number): boolean {
  return schema >= OLDEST_CANVAS_SURFACE_SCHEMA && schema <= NEWEST_CANVAS_SURFACE_SCHEMA;
}

/**
 * Keep only the declarations an answered schema can express.
 *
 * Schema two predates the backdrop descriptor. A schema that cannot express a surface keeps no
 * operation at all, so callers treat the result as an absent surface.
 */
export function canvasSurfaceCapabilitiesInSchema(
  value: CanvasSurfaceCapabilities,
  schema: number,
): CanvasSurfaceCapabilities {
  if (!expressesCanvasSurface(schema))
    return {
      schema,
      imageFormats: [],
      viewportCommands: [],
      viewportState: false,
      previewFormats: [],
      comparison: false,
      overview: false,
      input: false,
    };
  const common = {
    schema,
    imageFormats: value.imageFormats,
    viewportCommands: value.viewportCommands,
    viewportState: value.viewportState,
    previewFormats: value.previewFormats,
    comparison: value.comparison,
    overview: value.overview,
    input: value.input,
  };
  return schema >= 3 && value.backdrop ? { ...common, backdrop: value.backdrop } : common;
}
