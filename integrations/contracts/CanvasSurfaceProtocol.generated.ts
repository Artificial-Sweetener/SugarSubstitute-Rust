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
/** Describe the schema-defined CanvasSurfaceCapabilities wire value. */
export type CanvasSurfaceCapabilities = {
  readonly schema: 1;
  readonly imageFormats: readonly ('png' | 'jpeg' | 'webp')[];
  readonly viewportCommands: readonly ('fit' | 'native' | 'zoom' | 'pan')[];
  readonly viewportState: boolean;
  readonly previewFormats: readonly ('png' | 'jpeg')[];
  readonly comparison: boolean;
  readonly overview: boolean;
};
/** Describe the schema-defined CanvasSurfaceCapabilitiesRequest wire value. */
export type CanvasSurfaceCapabilitiesRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'surface.capabilities.request';
  readonly schema: 1;
};
/** Describe the schema-defined CanvasSurfaceCapabilitiesReply wire value. */
export type CanvasSurfaceCapabilitiesReply = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'surface.capabilities.reply';
  readonly status: 'available' | 'unsupported' | 'unavailable' | 'error';
  readonly capabilities: CanvasSurfaceCapabilities;
  readonly message: string;
};
/** Describe the schema-defined CanvasSurfaceAddress wire value. */
export type CanvasSurfaceAddress = {
  readonly sessionId: string;
  readonly viewId: string;
  readonly windowId: string;
  readonly attachmentId: string;
  readonly generation: number;
};
/** Describe the schema-defined CanvasSurfaceFileReference wire value. */
export type CanvasSurfaceFileReference = {
  readonly filename: string;
  readonly subfolder: string;
  readonly type: 'input' | 'output' | 'temp';
};
/** Describe the schema-defined CanvasSurfacePoint wire value. */
export type CanvasSurfacePoint = { readonly x: number; readonly y: number };
/** Describe the schema-defined CanvasSurfaceSize wire value. */
export type CanvasSurfaceSize = { readonly width: number; readonly height: number };
/** Describe the schema-defined CanvasSurfaceClip wire value. */
export type CanvasSurfaceClip = {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
};
/** Describe the schema-defined CanvasSurfaceGeometry wire value. */
export type CanvasSurfaceGeometry = {
  readonly coordinateSpace: 'client';
  readonly units: 'css-pixels';
  readonly origin: CanvasSurfacePoint;
  readonly viewport: CanvasSurfaceSize;
  readonly clip: CanvasSurfaceClip;
  readonly physicalPixelsPerCssPixel: number;
  readonly visible: boolean;
  readonly focused: boolean;
};
/** Describe the schema-defined CanvasSurfacePrepareRequest wire value. */
export type CanvasSurfacePrepareRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'surface.prepare';
  readonly address: CanvasSurfaceAddress;
  readonly revision: number;
};
/** Describe the schema-defined CanvasSurfaceGeometryRequest wire value. */
export type CanvasSurfaceGeometryRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'surface.geometry';
  readonly address: CanvasSurfaceAddress;
  readonly revision: number;
  readonly geometry: CanvasSurfaceGeometry;
};
/** Describe the schema-defined CanvasSurfaceSelectRequest wire value. */
export type CanvasSurfaceSelectRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'surface.select';
  readonly address: CanvasSurfaceAddress;
  readonly revision: number;
  readonly itemId: string;
  readonly file: CanvasSurfaceFileReference;
};
/** Describe the schema-defined CanvasSurfaceCommitRequest wire value. */
export type CanvasSurfaceCommitRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'surface.commit';
  readonly address: CanvasSurfaceAddress;
  readonly revision: number;
};
/** Describe the schema-defined CanvasSurfaceReleaseRequest wire value. */
export type CanvasSurfaceReleaseRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'surface.release';
  readonly address: CanvasSurfaceAddress;
  readonly revision: number;
};
/** Describe the schema-defined CanvasSurfaceCancelRequest wire value. */
export type CanvasSurfaceCancelRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'cancel';
};
/** Describe the schema-defined CanvasSurfaceReply wire value. */
export type CanvasSurfaceReply = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'surface.reply';
  readonly address: CanvasSurfaceAddress;
  readonly revision: number;
  readonly status: 'ready' | 'applied' | 'stale' | 'cancelled' | 'unavailable' | 'error';
  readonly message: string;
};
/** Describe the schema-defined CanvasSurfaceViewportCommand wire value. */
export type CanvasSurfaceViewportCommand =
  | { readonly type: 'read' }
  | { readonly type: 'fit' }
  | { readonly type: 'native' }
  | { readonly type: 'zoom'; readonly factor: number }
  | { readonly type: 'pan'; readonly x: number; readonly y: number; readonly units: 'viewport' };
/** Describe the schema-defined CanvasSurfaceImageDimensions wire value. */
export type CanvasSurfaceImageDimensions = { readonly width: number; readonly height: number };
/** Describe the schema-defined CanvasSurfaceViewportState wire value. */
export type CanvasSurfaceViewportState = {
  readonly mode: 'fit' | 'native' | 'custom';
  readonly physicalScale: number;
  readonly dimensions: CanvasSurfaceImageDimensions;
};
/** Describe the schema-defined CanvasSurfaceViewportRequest wire value. */
export type CanvasSurfaceViewportRequest = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'surface.viewport.request';
  readonly address: CanvasSurfaceAddress;
  readonly revision: number;
  readonly command: CanvasSurfaceViewportCommand;
};
/** Describe the schema-defined CanvasSurfaceViewportReply wire value. */
export type CanvasSurfaceViewportReply = {
  readonly protocol: 'sugarcubes.canvas';
  readonly version: 3;
  readonly session: string;
  readonly requestId: number;
  readonly type: 'surface.viewport.reply';
  readonly address: CanvasSurfaceAddress;
  readonly revision: number;
  readonly status: 'applied';
  readonly message: string;
  readonly viewport: CanvasSurfaceViewportState;
};
/** Describe the shared canvas surface messages. */
export type CanvasSurfaceMessage =
  | CanvasSurfaceCapabilitiesRequest
  | CanvasSurfaceCapabilitiesReply
  | CanvasSurfacePrepareRequest
  | CanvasSurfaceGeometryRequest
  | CanvasSurfaceSelectRequest
  | CanvasSurfaceCommitRequest
  | CanvasSurfaceReleaseRequest
  | CanvasSurfaceReply
  | CanvasSurfaceViewportRequest
  | CanvasSurfaceViewportReply;
