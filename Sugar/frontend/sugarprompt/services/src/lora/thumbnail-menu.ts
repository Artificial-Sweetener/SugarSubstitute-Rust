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
/** Pure prepared output-canvas hierarchy mirrors donor scene/batch/source grouping independently of DOM. */
import { requireValue } from '../../../core/src/required.js';
import type { LoraMetadataMenuItem, LoraOutputImage } from './metadata-actions-contracts.js';
import { compareScalarText } from '../text/scalar.js';
/** The caller owns immutable canonical request preparation; this owner supplies only grouping and labels. */
export type ThumbnailLeaf = (label: string, image: LoraOutputImage) => LoraMetadataMenuItem;
/** Retain first-seen source order and donor image numbering for repeated source/batch output. */
function sources(
  choices: readonly LoraOutputImage[],
  leaf: ThumbnailLeaf,
): readonly LoraMetadataMenuItem[] {
  return [...new Set(choices.map((value) => value.sourceKey))].map((key) => {
    const group = choices.filter((value) => value.sourceKey === key),
      label = requireValue(group[0], 'thumbnail-menu').sourceLabel || 'Output';
    return group.length === 1
      ? leaf(label, requireValue(group[0], 'thumbnail-menu'))
      : {
          kind: 'submenu',
          label,
          children: group.map((value, index) => leaf(`Image ${index + 1}`, value)),
        };
  });
}
/** Multiple numeric sets appear in ascending donor batch order; singleton groups collapse to direct actions. */
function batches(
  choices: readonly LoraOutputImage[],
  leaf: ThumbnailLeaf,
): readonly LoraMetadataMenuItem[] {
  const indices = [...new Set(choices.map((value) => value.setIndex))].sort((a, b) => a - b);
  if (indices.length <= 1) return sources(choices, leaf);
  return indices.map((index) => {
    const group = choices.filter((value) => value.setIndex === index),
      label = `Batch ${index}`;
    return group.length === 1
      ? leaf(label, requireValue(group[0], 'thumbnail-menu'))
      : { kind: 'submenu', label, children: sources(group, leaf) };
  });
}
/** Zero/one/many images and an active image retain the donor's adaptive menu shape. */
export function prepareThumbnailMenu(
  choices: readonly LoraOutputImage[],
  activeImageId: string | null,
  leaf: ThumbnailLeaf,
): LoraMetadataMenuItem {
  const label = 'Set thumbnail from canvas';
  if (!choices.length) return { kind: 'disabled', label };
  if (choices.length === 1) return leaf(label, requireValue(choices[0], 'thumbnail-menu'));
  const keys = new Set(choices.map((value) => value.sceneKey).filter(Boolean));
  const meaningful =
    keys.size > 1 ||
    (keys.size > 0 && choices.some((value) => value.sceneTitle && value.sceneTitle !== 'Scene'));
  const sorted = meaningful
    ? [...choices].sort(
        (a, b) =>
          a.sceneOrder - b.sceneOrder ||
          compareScalarText(a.sceneTitle, b.sceneTitle) ||
          compareScalarText(a.sceneKey, b.sceneKey),
      )
    : choices;
  const hierarchy: readonly LoraMetadataMenuItem[] = meaningful
    ? [...new Set(sorted.map((value) => value.sceneKey))]
        .map((key) => sorted.filter((value) => value.sceneKey === key))
        .map((group) => {
          const sceneLabel = requireValue(group[0], 'thumbnail-menu').sceneTitle || 'Scene';
          return group.length === 1
            ? leaf(sceneLabel, requireValue(group[0], 'thumbnail-menu'))
            : {
                kind: 'submenu',
                label: sceneLabel,
                children: batches(group, leaf),
              };
        })
    : batches(choices, leaf);
  const active = choices.find((value) => value.imageId === activeImageId);
  return {
    kind: 'submenu',
    label,
    children: active
      ? [leaf('Current image', active), { kind: 'separator' }, ...hierarchy]
      : hierarchy,
  };
}
