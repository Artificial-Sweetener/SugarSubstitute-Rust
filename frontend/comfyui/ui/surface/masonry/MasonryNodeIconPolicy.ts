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
/** Resolve Substitute's semantic title-icon choices for masonry without altering node state. */
import type { ComfyNode } from '../../types/graph.js';
import { cubeFacePromptRole, findCubeFacePromptWidget } from '../CubeFacePromptPolicy.js';
import { isCubeFaceSamplerWorker } from '../CubeFaceNodeSemantics.js';

export type MasonryNodeIcon =
  | 'application'
  | 'edit'
  | 'eraser'
  | 'folder'
  | 'model'
  | 'palette'
  | 'photo';

const classIcons: Readonly<Record<string, MasonryNodeIcon>> = {
  LoadImage: 'folder',
  LoadImageMask: 'folder',
  'SimpleSyrup.LoadMaskBatch': 'folder',
  EmptyLatentImage: 'photo',
  KSampler: 'application',
  DetailerForEach: 'application',
  'SimpleSyrup.PromptEncodeStyle': 'edit',
  VectorscopeCC: 'palette',
  VAELoader: 'model',
  CheckpointLoaderSimple: 'model',
};

const modelClasses = new Set([
  'UpscaleModelLoader',
  'UltralyticsDetectorProvider',
  'SimpleSyrup.LoadUltralyticsModel',
]);

/** Prefer prompt/class semantics and sampler contracts before a model-picker fallback. */
export function resolveMasonryNodeIcon(node: ComfyNode): MasonryNodeIcon | null {
  if (findCubeFacePromptWidget(node))
    return cubeFacePromptRole(node) === 'negative' ? 'eraser' : 'edit';
  const classType = (node.class_type ?? node.type ?? '').trim();
  const explicit = Object.hasOwn(classIcons, classType) ? classIcons[classType] : undefined;
  if (explicit) return explicit;
  if (isCubeFaceSamplerWorker(node)) return 'application';
  if (
    (node.widgets ?? []).some((widget) => {
      if (widget.type !== 'combo' && !Array.isArray(widget.options?.values)) return false;
      const name = widget.name.trim().toLowerCase();
      return (
        name === 'vae_name' ||
        (name === 'vae' && classType.startsWith('SimpleSyrup.')) ||
        (name === 'model_name' && modelClasses.has(classType)) ||
        /checkpoint|ckpt|lora|unet|diffusion|control_?net|clip|text_encoder/.test(name)
      );
    })
  )
    return 'model';
  return null;
}
