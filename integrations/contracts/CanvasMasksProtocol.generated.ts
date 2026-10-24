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
/** Generated from masks.schema.json by scripts/integrations/generate-masks-contract.ts. */
/** Describe the schema-defined CanvasDocument wire value. */
export type CanvasDocument = {
  readonly id: string;
  readonly geometryRevision: string;
  readonly width: number;
  readonly height: number;
};
/** Describe the schema-defined CanvasMaskPixels wire value. */
export type CanvasMaskPixels = {
  readonly width: number;
  readonly height: number;
  readonly data: string;
};
/** Describe the schema-defined CanvasMaskReference wire value. */
export type CanvasMaskReference = {
  readonly canvasId: string;
  readonly document: CanvasDocument;
  readonly id: string;
};
/** Describe the schema-defined CanvasMaskSnapshot wire value. */
export type CanvasMaskSnapshot = {
  readonly mask: CanvasMaskReference;
  readonly revision: string;
  readonly pixels: CanvasMaskPixels;
};
/** Describe the schema-defined CanvasMaskContext wire value. */
export type CanvasMaskContext = {
  readonly kind: 'context';
  readonly supported: boolean;
  readonly document: CanvasDocument | null;
};
/** Describe the schema-defined CanvasMaskCreate wire value. */
export type CanvasMaskCreate = {
  readonly type: 'create';
  readonly document: CanvasDocument;
  readonly name: string;
  readonly source: 'selection' | 'blank' | 'pixels';
  readonly pixels: CanvasMaskPixels | null;
};
/** Describe the schema-defined CanvasMaskActivate wire value. */
export type CanvasMaskActivate = { readonly type: 'activate'; readonly mask: CanvasMaskReference };
/** Describe the schema-defined CanvasMaskCapture wire value. */
export type CanvasMaskCapture = {
  readonly type: 'capture';
  readonly masks: readonly CanvasMaskReference[];
  readonly maximumSize: number;
};
/** Describe the schema-defined CanvasMaskCommand wire value. */
export type CanvasMaskCommand =
  | { readonly type: 'context' }
  | CanvasMaskCreate
  | CanvasMaskActivate
  | CanvasMaskCapture
  | { readonly type: 'release'; readonly mask: CanvasMaskReference | null };
/** Describe the schema-defined CanvasMaskResult wire value. */
export type CanvasMaskResult =
  | CanvasMaskContext
  | { readonly kind: 'created'; readonly mask: CanvasMaskReference }
  | { readonly kind: 'activated' }
  | { readonly kind: 'captured'; readonly snapshots: readonly CanvasMaskSnapshot[] }
  | {
      readonly kind: 'failure';
      readonly status: 'unavailable' | 'unsupported' | 'stale' | 'cancelled' | 'error';
      readonly message: string;
    };
/** Describe the schema-defined CanvasMaskRequest wire value. */
export type CanvasMaskRequest = {
  readonly protocol: 'sugarcubes.canvas.masks';
  readonly version: 1;
  readonly session: string;
  readonly requestId: number;
  readonly command: CanvasMaskCommand;
};
/** Describe the schema-defined CanvasMaskReply wire value. */
export type CanvasMaskReply = {
  readonly protocol: 'sugarcubes.canvas.masks';
  readonly version: 1;
  readonly session: string;
  readonly requestId: number;
  readonly result: CanvasMaskResult;
};
/** Describe the schema-defined CanvasMaskChanged wire value. */
export type CanvasMaskChanged = {
  readonly protocol: 'sugarcubes.canvas.masks';
  readonly version: 1;
  readonly session: string;
  readonly type: 'changed';
  readonly documentId: string;
  readonly revision: string;
  readonly maskId: string;
};
/** Describe every version-one mask message. */
export type CanvasMaskMessage = CanvasMaskRequest | CanvasMaskReply | CanvasMaskChanged;
