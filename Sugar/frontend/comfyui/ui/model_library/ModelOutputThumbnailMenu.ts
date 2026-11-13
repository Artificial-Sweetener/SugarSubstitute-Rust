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
/** Adopt Substitute's adaptive scene, batch and output-image thumbnail menu. */
import type { ModelContextEntry } from './ModelContextMenu.js';
import type { ModelOutputThumbnailChoice } from './ModelOutputThumbnails.js';

/** Preserve Python's Unicode scalar ordering independently of the browser's locale. */
function compareSceneText(left: string, right: string): number {
  const leftPoints = Array.from(left, (character) => character.codePointAt(0) ?? 0);
  const rightPoints = Array.from(right, (character) => character.codePointAt(0) ?? 0);
  for (let index = 0; index < Math.max(leftPoints.length, rightPoints.length); index++) {
    const difference = (leftPoints[index] ?? -1) - (rightPoints[index] ?? -1);
    if (difference) return difference;
  }
  return 0;
}

/** Collapse a singleton group to its concrete image action. */
function group(
  label: string,
  choices: readonly ModelOutputThumbnailChoice[],
  children: () => readonly ModelContextEntry[],
): ModelContextEntry {
  const only = choices.length === 1 ? choices[0] : undefined;
  return only
    ? { label, enabled: true, command: 'output-thumbnail', url: null, imageId: only.id }
    : { label, enabled: true, command: null, url: null, children: children() };
}

/** Keep source order and number concrete images only within their source. */
function sources(choices: readonly ModelOutputThumbnailChoice[]): readonly ModelContextEntry[] {
  return [...new Set(choices.map((choice) => choice.sourceKey))].map((key) => {
    const selected = choices.filter((choice) => choice.sourceKey === key);
    return group(selected[0]?.sourceLabel || 'Output', selected, () =>
      selected.map((choice, index) => ({
        label: `Image ${String(index + 1)}`,
        enabled: true,
        command: 'output-thumbnail',
        url: null,
        imageId: choice.id,
      })),
    );
  });
}

/** Omit the batch level when all choices belong to one batch. */
function batches(choices: readonly ModelOutputThumbnailChoice[]): readonly ModelContextEntry[] {
  const indices = [...new Set(choices.map((choice) => choice.batch))].sort((a, b) => a - b);
  return indices.length <= 1
    ? sources(choices)
    : indices.map((index) => {
        const selected = choices.filter((choice) => choice.batch === index);
        return group(`Batch ${String(index)}`, selected, () => sources(selected));
      });
}

/** Match disabled, direct and hierarchical actions without manufacturing a current image. */
export function modelOutputThumbnailMenu(
  choices: readonly ModelOutputThumbnailChoice[],
  active: ModelOutputThumbnailChoice | null,
): ModelContextEntry {
  const label = 'Set thumbnail from canvas';
  if (!choices.length) return { label, enabled: false, command: 'output-thumbnail', url: null };
  return group(label, choices, () => {
    const children: ModelContextEntry[] = [];
    if (active && choices.some((choice) => choice.id === active.id)) {
      children.push({
        label: 'Current image',
        enabled: true,
        command: 'output-thumbnail',
        url: null,
        imageId: active.id,
      });
      children.push({ label: '', enabled: false, command: null, url: null, separator: true });
    }
    const keys = new Set(
      choices.filter((choice) => choice.sceneKey).map((choice) => choice.sceneKey),
    );
    const meaningful =
      keys.size > 1 ||
      (keys.size > 0 &&
        choices.some((choice) => choice.sceneTitle && choice.sceneTitle !== 'Scene'));
    if (!meaningful) return [...children, ...batches(choices)];
    const sorted = [...choices].sort(
      (a, b) =>
        a.sceneOrder - b.sceneOrder ||
        compareSceneText(a.sceneTitle, b.sceneTitle) ||
        compareSceneText(a.sceneKey, b.sceneKey),
    );
    for (const key of new Set(sorted.map((choice) => choice.sceneKey))) {
      const selected = sorted.filter((choice) => choice.sceneKey === key);
      children.push(group(selected[0]?.sceneTitle || 'Scene', selected, () => batches(selected)));
    }
    return children;
  });
}
