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
/** Generated from integrations/gimp/native.schema.json by scripts/gimp/NativeContract.ts. */
/** Describe the schema-defined NativeId wire value. */
export type NativeId = number;
/** Describe the schema-defined NativeTattoo wire value. */
export type NativeTattoo = number;
/** Describe the schema-defined NativeToken wire value. */
export type NativeToken = string;
/** Describe the schema-defined NativeDocument wire value. */
export type NativeDocument = {
  readonly imageId: NativeId;
  readonly revision: string;
  readonly width: CanvasWidth;
  readonly height: CanvasHeight;
  readonly name: string;
};
/** Describe the schema-defined NativeLayer wire value. */
export type NativeLayer = {
  readonly tattoo: NativeTattoo;
  readonly parentTattoo: NativeTattoo | null;
  readonly name: string;
  readonly depth: number;
  readonly visible: boolean;
  readonly hasMask: boolean;
};
/** Describe the schema-defined NativeLayers wire value. */
export type NativeLayers = readonly NativeLayer[];
/** Describe the schema-defined NativeVisibility wire value. */
export type NativeVisibility = { readonly tattoo: NativeTattoo; readonly visible: boolean };
/** Describe the schema-defined NativeCommand wire value. */
export type NativeCommand =
  | { readonly type: 'context' }
  | { readonly type: 'snapshot.begin'; readonly document: NativeDocument }
  | { readonly type: 'snapshot.release'; readonly token: NativeToken }
  | {
      readonly type: 'snapshot.selection';
      readonly token: NativeToken;
      readonly width: RasterWidth;
      readonly height: RasterHeight;
      readonly scale: number;
    }
  | {
      readonly type: 'snapshot.projection';
      readonly token: NativeToken;
      readonly width: RasterWidth;
      readonly height: RasterHeight;
      readonly scale: number;
      readonly visibility: readonly NativeVisibility[];
    }
  | {
      readonly type: 'channel.create';
      readonly document: NativeDocument;
      readonly name: string;
      readonly source: 'selection' | 'blank' | 'pixels';
      readonly data: RasterData | null;
    }
  | {
      readonly type: 'channel.select';
      readonly document: NativeDocument;
      readonly tattoo: NativeTattoo;
    }
  | { readonly type: 'channel.release'; readonly tattoo: NativeTattoo | null }
  | {
      readonly type: 'snapshot.channel';
      readonly token: NativeToken;
      readonly tattoo: NativeTattoo;
      readonly width: RasterWidth;
      readonly height: RasterHeight;
      readonly scale: number;
    }
  | {
      readonly type: 'image.insert';
      readonly document: NativeDocument;
      readonly data: PngData;
      readonly width: number;
      readonly height: number;
      readonly name: string;
      readonly left: number;
      readonly top: number;
    };
/** Describe the schema-defined NativeResult wire value. */
export type NativeResult =
  | { readonly kind: 'channel-created'; readonly tattoo: NativeTattoo; readonly committed: true }
  | { readonly kind: 'channel-selected'; readonly committed: true }
  | {
      readonly kind: 'context';
      readonly hostId: NativeToken;
      readonly document: NativeDocument | null;
      readonly layers: NativeLayers;
      readonly primaryLayer: NativeTattoo | null;
      readonly rgb: boolean;
    }
  | {
      readonly kind: 'snapshot';
      readonly token: NativeToken;
      readonly document: NativeDocument;
      readonly layers: NativeLayers;
      readonly primaryLayer: NativeTattoo | null;
    }
  | { readonly kind: 'released' }
  | {
      readonly kind: 'pixels';
      readonly width: RasterWidth;
      readonly height: RasterHeight;
      readonly format: 'rgba8' | 'coverage8';
      readonly data: RasterData;
    }
  | {
      readonly kind: 'inserted';
      readonly committed: true;
      readonly tattoo: NativeTattoo;
      readonly left: number;
      readonly top: number;
      readonly width: number;
      readonly height: number;
    }
  | {
      readonly kind: 'failure';
      readonly status: 'cancelled' | 'stale' | 'unsupported' | 'error';
      readonly message: string;
    };
/** Describe the schema-defined NativeRequest wire value. */
export type NativeRequest = {
  readonly type: 'request';
  readonly id: NativeId;
  readonly command: NativeCommand;
};
/** Describe the schema-defined NativeCancel wire value. */
export type NativeCancel = { readonly type: 'cancel'; readonly id: NativeId };
/** Describe the schema-defined NativeReply wire value. */
export type NativeReply = {
  readonly type: 'reply';
  readonly id: NativeId;
  readonly result: NativeResult;
};
/** Describe the schema-defined CanvasWidth wire value. */
export type CanvasWidth = number;
/** Describe the schema-defined CanvasHeight wire value. */
export type CanvasHeight = number;
/** Describe the schema-defined RasterWidth wire value. */
export type RasterWidth = number;
/** Describe the schema-defined RasterHeight wire value. */
export type RasterHeight = number;
/** Describe the schema-defined PngData wire value. */
export type PngData = string;
/** Describe the schema-defined RasterData wire value. */
export type RasterData = string;
/** Describe private GIMP native primitive messages. */
export type NativeMessage = NativeRequest | NativeCancel | NativeReply;
