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
/** Recognize file-loading fields through native upload metadata independently of canvas representation. */
export type InputKind = 'image' | 'mask';
/** Describe one independently configurable upload field on an installed node. */
export interface InputLoader {
  readonly kind: InputKind;
  readonly classType: string;
  readonly field: string;
  readonly ordered: boolean;
  readonly channel: string | null;
}
/** Receive validated native source metadata without leaking host widgets into domain policy. */
export interface InputLoaderDefinition {
  readonly classType: string;
  readonly outputs: readonly string[];
  readonly fields: readonly {
    name: string;
    upload: boolean;
    folder: string | null;
    ordered: boolean;
  }[];
  readonly channel: string | null;
}
/** Preserve known mask semantics and expose each image upload field as an independent source. */
export function recognizeInputLoaders(definition: InputLoaderDefinition): readonly InputLoader[] {
  const { classType, channel } = definition;
  if (classType === 'SimpleSyrup.LoadMaskBatch' || classType === 'LoadImageMask')
    return [
      {
        kind: 'mask',
        classType,
        field: 'image',
        ordered: classType === 'SimpleSyrup.LoadMaskBatch',
        channel: 'channel',
      },
    ];
  if (
    classType === 'SimpleSyrup.LoadImageList' ||
    classType === 'LoadImage' ||
    classType === 'LoadImageOutput'
  )
    return [
      {
        kind: 'image',
        classType,
        field: 'image',
        ordered: classType === 'SimpleSyrup.LoadImageList',
        channel: null,
      },
    ];
  const mask = definition.outputs.includes('MASK') && !definition.outputs.includes('IMAGE');
  const fields = definition.fields.filter(
    (field) =>
      field.upload &&
      (mask
        ? field.folder === null || field.folder === 'input'
        : field.folder === null || ['input', 'output', 'temp'].includes(field.folder)),
  );
  if (mask && fields.length !== 1) return [];
  return fields.map((field) => ({
    kind: mask ? 'mask' : 'image',
    classType,
    field: field.name,
    ordered: field.ordered,
    channel: mask ? channel : null,
  }));
}
/** Keep source labels consistent across initial state, ordered controls and queue preparation. */
export function inputLabel(kind: InputKind): string {
  return kind === 'mask' ? 'Mask' : 'Image';
}
/** Describe native file interpretation without tying the source itself to an image or mask consumer. */
export interface InputPreviewSettings {
  readonly kind: InputKind;
  readonly classType: string;
  readonly field: string;
  readonly inputs: Readonly<Record<string, string | number | boolean>>;
}
