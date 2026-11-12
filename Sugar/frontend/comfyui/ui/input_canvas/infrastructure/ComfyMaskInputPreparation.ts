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
/** Adapt exclusive authored loader pairs to shared pixel preparation and canonical output replacement. */
import type { WorkflowPreparedInput } from '../../execution/domain/WorkflowPreparation.js';
import type { InputRasterPort } from '../application/InputRasterPort.js';
import {
  preparedSourceFile,
  type PreparedInputSources,
  type InputSourceAssets,
} from '../application/InputSourcePreparation.js';
import type { MaskInputPreparation } from '../application/MaskInputPreparation.js';
import type { PreparedInputPixels } from '../application/PreparedInputPixels.js';
import { nativeMaskSource, maskSourceImage } from '../domain/MaskSourcePixels.js';
import { DEFAULT_MASK_EDITS, hasMaskEdits } from '../domain/MaskEditSettings.js';
import type { InputAssetPorts } from '../domain/InputLoader.js';
import type { InputWorkflowTarget } from './InputWorkflowProjection.js';
import { sameCanvasSourceDocument } from '../../core/embedded/CanvasRuntime.js';

/** Keep authored file substitutions separate from execution-only decoded mask outputs. */
export interface PreparedMaskInputs {
  readonly sourceFiles: ReadonlyMap<string, string>;
  readonly returnInputs: ReadonlyMap<string, PreparedInputPixels>;
  readonly replacements: readonly WorkflowPreparedInput[];
}

/** Own native decoding and immutable upload translation while the application owns coordinated pixel processing. */
export class ComfyMaskInputPreparation {
  /** Reuse source codecs, uploads and graph relationships already used by normal source preparation. */
  constructor(
    private readonly rasters: InputRasterPort,
    private readonly assets: InputSourceAssets,
    private readonly pixels: MaskInputPreparation,
    readonly pairs: () => ReadonlyMap<string, string>,
  ) {}

  /** Prepare each eligible scalar pair once, preserving image-node operations and custom mask interpretation. */
  async prepare(
    targets: readonly InputWorkflowTarget[],
    sources: PreparedInputSources,
    pairs: ReadonlyMap<string, string>,
    ports: ReadonlyMap<string, InputAssetPorts>,
    fallbackFrame: { readonly width: number; readonly height: number } | null,
  ): Promise<PreparedMaskInputs> {
    const sourceFiles = new Map<string, string>(),
      returnInputs = new Map<string, PreparedInputPixels>();
    const replacements: WorkflowPreparedInput[] = [];
    const byKey = new Map(targets.map((target) => [target.key, target]));
    for (const [maskKey, imageKey] of pairs) {
      const mask = byKey.get(maskKey),
        image = byKey.get(imageKey);
      const settings = mask?.binding.edits ?? DEFAULT_MASK_EDITS;
      if (!mask || !image || !hasMaskEdits(settings)) continue;
      if (
        mask.loader.kind !== 'mask' ||
        image.loader.kind !== 'image' ||
        mask.loader.ordered ||
        image.loader.ordered ||
        mask.binding.entries.length !== 1 ||
        image.binding.entries.length !== 1 ||
        image.maskOutput
      )
        throw new Error('Mask editing requires one image and one mask.');
      if (mask.unresolvedSettings.length)
        throw new Error('Mask editing requires captured loader settings.');
      const maskEntry = mask.binding.entries[0],
        imageEntry = image.binding.entries[0];
      if (!maskEntry || !imageEntry) throw new Error('The edited input source is unavailable.');
      const maskFile = preparedSourceFile(mask.key, maskEntry, sources),
        imageFile = preparedSourceFile(image.key, imageEntry, sources);
      const maskSnapshot = sources.snapshots.get(JSON.stringify([mask.key, maskEntry.id]));
      const imageSnapshot = sources.snapshots.get(JSON.stringify([image.key, imageEntry.id]));
      if (
        maskSnapshot &&
        imageSnapshot &&
        (maskEntry.source.kind !== 'canvas' ||
          imageEntry.source.kind !== 'canvas' ||
          maskEntry.source.canvasId !== imageEntry.source.canvasId ||
          !sameCanvasSourceDocument(maskSnapshot.document, imageSnapshot.document))
      )
        throw new Error('The edited image and mask belong to different canvas documents.');
      const coverage =
        (maskSnapshot
          ? nativeMaskSource(maskSnapshot.pixels, mask.loader, mask.settings, false)
          : null) ?? (await this.rasters.load(maskFile, mask.settings, 0));
      const originalImage =
        imageSnapshot?.pixels ?? (await this.rasters.load(imageFile, image.settings, 0));
      const frame = imageSnapshot?.document ?? maskSnapshot?.document ?? fallbackFrame;
      if (!frame || originalImage.width !== frame.width || originalImage.height !== frame.height)
        throw new Error('The edited inputs have no declared mapping to the canvas frame.');
      const prepared = await this.pixels.prepare(originalImage, coverage, settings);
      const croppedFile =
        prepared.image === originalImage ? imageFile : await this.assets.stage(prepared.image);
      const editedFile = await this.assets.stage(maskSourceImage(prepared.mask.mask));
      sourceFiles.set(image.key, croppedFile);
      returnInputs.set(image.key, {
        file: croppedFile,
        classType: image.loader.classType,
        field: image.loader.field,
        pixels: prepared.image,
        mapping: prepared.mask.mapping,
        sourceSize: { width: originalImage.width, height: originalImage.height },
      });
      returnInputs.set(mask.key, {
        file: editedFile,
        classType: 'LoadImageMask',
        field: 'image',
        pixels: prepared.mask.mask,
        mapping: prepared.mask.mapping,
        sourceSize: { width: originalImage.width, height: originalImage.height },
      });
      const outputs = ports.get(mask.loader.classType)?.outputs ?? [];
      const slots = outputs.flatMap((kind, slot) => (kind === 'mask' ? [slot] : []));
      if (slots.length !== 1 || slots[0] === undefined)
        throw new Error('The mask loader has ambiguous outputs.');
      replacements.push({
        nodePath: mask.nodePath,
        classType: mask.loader.classType,
        field: mask.loader.field,
        file: maskFile,
        slot: slots[0],
        kind: 'mask',
        preparedFile: editedFile,
      });
    }
    return { sourceFiles, returnInputs, replacements: Object.freeze(replacements) };
  }
}
