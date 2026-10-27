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
/** Generated from canvas.schema.json by scripts/integrations/generate-canvas-contract.ts. */
/** Describe the schema-defined CanvasCapabilities wire value. */
export type CanvasCapabilities =
  | {
      readonly displayOutput: boolean;
      readonly insertImageLayer: boolean;
      readonly positionImageLayer: boolean;
    }
  | {
      readonly displayOutput: boolean;
      readonly insertImageLayer: boolean;
      readonly positionImageLayer: boolean;
      readonly geometry: boolean;
    };
/** Describe the schema-defined CanvasDescriptor wire value. */
export type CanvasDescriptor = {
  readonly id: string;
  readonly label: string;
  readonly capabilities: CanvasCapabilities;
};
/** Describe the schema-defined CanvasDocument wire value. */
export type CanvasDocument = {
  readonly id: string;
  readonly geometryRevision: string;
  readonly width: number;
  readonly height: number;
};
/** Describe the schema-defined CanvasFailure wire value. */
export type CanvasFailure = {
  readonly status: 'unavailable' | 'unsupported' | 'stale' | 'cancelled' | 'error';
  readonly message: string;
};
/** Describe the schema-defined CanvasInsertedLayer wire value. */
export type CanvasInsertedLayer = {
  readonly status: 'inserted';
  readonly documentId: string;
  readonly layerId: string;
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
};
/** Describe the schema-defined CanvasInsertResult wire value. */
export type CanvasInsertResult = CanvasInsertedLayer | CanvasFailure;
/** Describe the schema-defined CanvasContext wire value. */
export type CanvasContext = {
  readonly canvas: CanvasDescriptor;
  readonly document: CanvasDocument | null;
};
/** Describe the schema-defined CanvasInsertImage wire value. */
export type CanvasInsertImage = {
  readonly document: CanvasDocument;
  readonly image: CanvasEncodedImage;
  readonly layerName: string;
  readonly position: null | CanvasImagePosition;
};
/** Describe the schema-defined CanvasReadRequest wire value. */
export type CanvasReadRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'context.request';
};
/** Describe the schema-defined CanvasReadReply wire value. */
export type CanvasReadReply = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'context.reply';
  readonly context: CanvasContext;
};
/** Describe the schema-defined CanvasInsertRequest wire value. */
export type CanvasInsertRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'image.insert';
  readonly image: CanvasInsertImage;
};
/** Describe the schema-defined CanvasInsertReply wire value. */
export type CanvasInsertReply = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'image.reply';
  readonly result: CanvasInsertResult;
};
/** Describe the schema-defined CanvasCancelRequest wire value. */
export type CanvasCancelRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'cancel';
};
/** Describe the schema-defined CanvasErrorReply wire value. */
export type CanvasErrorReply = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'error.reply';
  readonly failure: CanvasFailure;
};
/** Describe the schema-defined CanvasEncodedImage wire value. */
export type CanvasEncodedImage = {
  readonly format: 'png';
  readonly encoding: 'base64';
  readonly data: string;
};
/** Describe the schema-defined CanvasSourceTarget wire value. */
export type CanvasSourceTarget =
  | { readonly kind: 'selection' }
  | { readonly kind: 'current-layer' }
  | { readonly kind: 'layer'; readonly layerId: string }
  | { readonly kind: 'composition' }
  | { readonly kind: 'composition-from-layer'; readonly layerId: string };
/** Describe the schema-defined CanvasInputTarget wire value. */
export type CanvasInputTarget =
  | CanvasSourceTarget
  | { readonly kind: 'file'; readonly file: string | null };
/** Describe the schema-defined CanvasPixelRaster wire value. */
export type CanvasPixelRaster = {
  readonly width: number;
  readonly height: number;
  readonly format: 'rgba8' | 'coverage8';
  readonly data: string;
};
/** Describe the schema-defined CanvasSourceLayer wire value. */
export type CanvasSourceLayer = {
  readonly id: string;
  readonly name: string;
  readonly parentId: string | null;
  readonly depth: number;
  readonly visible: boolean;
};
/** Describe the schema-defined CanvasSourceContext wire value. */
export type CanvasSourceContext = {
  readonly kind: 'context';
  readonly supported: boolean;
  readonly document: CanvasDocument | null;
  readonly revision: string;
  readonly targets: readonly (
    | 'selection'
    | 'current-layer'
    | 'layer'
    | 'composition'
    | 'composition-from-layer'
  )[];
  readonly layers: readonly CanvasSourceLayer[];
  readonly currentLayerId: string | null;
  readonly observesChanges: boolean;
};
/** Describe the schema-defined CanvasSourceSnapshot wire value. */
export type CanvasSourceSnapshot = {
  readonly target: CanvasSourceTarget;
  readonly document: CanvasDocument;
  readonly revision: string;
  readonly pixels: CanvasPixelRaster;
};
/** Describe the schema-defined CanvasSourceCommand wire value. */
export type CanvasSourceCommand =
  | CanvasSourceQuery
  | { readonly type: 'watch'; readonly enabled: boolean };
/** Describe the schema-defined CanvasSourceResult wire value. */
export type CanvasSourceResult =
  | CanvasSourceContext
  | { readonly kind: 'captured'; readonly snapshots: readonly CanvasSourceSnapshot[] }
  | { readonly kind: 'watching'; readonly supported: boolean }
  | {
      readonly kind: 'failure';
      readonly status: 'unavailable' | 'unsupported' | 'stale' | 'cancelled' | 'error';
      readonly message: string;
    };
/** Describe the schema-defined CanvasSourceRequest wire value. */
export type CanvasSourceRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'source.request';
  readonly command: CanvasSourceCommand;
};
/** Describe the schema-defined CanvasSourceReply wire value. */
export type CanvasSourceReply = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'source.reply';
  readonly result: CanvasSourceResult;
};
/** Describe the schema-defined CanvasSourceChanged wire value. */
export type CanvasSourceChanged = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: 0;
  readonly type: 'source.changed';
  readonly change: CanvasSourceChange;
};
/** Describe the schema-defined CanvasSourceChange wire value. */
export type CanvasSourceChange = { readonly documentId: string | null; readonly revision: string };
/** Describe the schema-defined CanvasSourceQuery wire value. */
export type CanvasSourceQuery =
  | { readonly type: 'context' }
  | {
      readonly type: 'capture';
      readonly document: CanvasDocument;
      readonly targets: readonly CanvasSourceTarget[];
      readonly maximumSize: number;
    };
/** Describe the schema-defined CanvasInputSource wire value. */
export type CanvasInputSource =
  | { readonly kind: 'file'; readonly file: string | null }
  | {
      readonly kind: 'canvas';
      readonly canvasId: string;
      readonly documentId: string | null;
      readonly target: CanvasSourceTarget;
    };
/** Describe the schema-defined CanvasImagePosition wire value. */
export type CanvasImagePosition = { readonly left: number; readonly top: number };
/** Describe the schema-defined CanvasDimensions wire value. */
export type CanvasDimensions = { readonly width: number; readonly height: number };
/** Describe the schema-defined CanvasDimensionFieldValues wire value. */
export type CanvasDimensionFieldValues = { readonly width: number; readonly height: number };
/** Describe the schema-defined CanvasSelectionBounds wire value. */
export type CanvasSelectionBounds = {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
};
/** Describe the schema-defined CanvasDimensionLimits wire value. */
export type CanvasDimensionLimits = {
  readonly minimum: number;
  readonly maximum: number;
  readonly multiple: number;
};
/** Describe the schema-defined CanvasScaleOutput wire value. */
export type CanvasScaleOutput = { readonly id: string; readonly label: string };
/** Describe the schema-defined CanvasGeometryCommand wire value. */
export type CanvasGeometryCommand =
  | { readonly type: 'context' }
  | { readonly type: 'resize'; readonly document: CanvasDocument; readonly size: CanvasDimensions };
/** Describe the schema-defined CanvasGeometryResult wire value. */
export type CanvasGeometryResult =
  | {
      readonly kind: 'context';
      readonly supported: boolean;
      readonly resize: boolean;
      readonly selectionSupported: boolean;
      readonly document: CanvasDocument | null;
      readonly selection: CanvasSelectionBounds | null;
    }
  | { readonly kind: 'resized'; readonly document: CanvasDocument }
  | {
      readonly kind: 'failure';
      readonly status: 'unavailable' | 'unsupported' | 'stale' | 'cancelled' | 'error';
      readonly message: string;
    };
/** Describe the schema-defined CanvasGeometryRequest wire value. */
export type CanvasGeometryRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'geometry.request';
  readonly command: CanvasGeometryCommand;
};
/** Describe the schema-defined CanvasGeometryReply wire value. */
export type CanvasGeometryReply = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'geometry.reply';
  readonly result: CanvasGeometryResult;
};
/** Describe every version-three canvas message. */
export type CanvasMessage =
  | CanvasReadRequest
  | CanvasReadReply
  | CanvasInsertRequest
  | CanvasInsertReply
  | CanvasCancelRequest
  | CanvasErrorReply
  | CanvasSourceRequest
  | CanvasSourceReply
  | CanvasSourceChanged
  | CanvasGeometryRequest
  | CanvasGeometryReply;
