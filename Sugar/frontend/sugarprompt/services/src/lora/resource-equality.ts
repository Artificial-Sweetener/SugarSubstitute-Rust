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
 * Whether two resolutions of a prompt's LoRA names show the same thing. A library publishes a new
 * catalog for every change to any model, and a prompt resolves its names again against each one;
 * almost always the answer is the same models with the same words and the same pictures. A host
 * compares before it publishes, so an answer that changed nothing redraws nothing.
 */
import type { LoraResource, LoraResourceSnapshot } from './contracts.js';

/** The same outcome for the same name: the same model, title, words, picture and page. */
function sameResource(left: LoraResource, right: LoraResource | undefined): boolean {
  if (!right || left.name !== right.name || left.resolution !== right.resolution) return false;
  if (left.resolution === 'ambiguous')
    return right.resolution === 'ambiguous' && left.candidateCount === right.candidateCount;
  if (left.resolution !== 'found' || right.resolution !== 'found') return true;
  return (
    left.model.backendValue === right.model.backendValue &&
    left.model.promptName === right.model.promptName &&
    left.model.basename === right.model.basename &&
    left.title === right.title &&
    left.subtitle === right.subtitle &&
    left.modelPageUrl === right.modelPageUrl &&
    // A catalog keeps the descriptor of a picture it already described, so a changed picture
    // is a different descriptor.
    left.thumbnail === right.thumbnail &&
    left.triggerWords.length === right.triggerWords.length &&
    left.triggerWords.every((word, index) => word === right.triggerWords[index])
  );
}

/** Compare two snapshots by what they resolve, not by which catalog resolved them. */
export function sameLoraResources(
  left: LoraResourceSnapshot,
  right: LoraResourceSnapshot,
): boolean {
  return (
    left === right ||
    (left.status === right.status &&
      left.resources.length === right.resources.length &&
      left.resources.every((resource, index) => sameResource(resource, right.resources[index])))
  );
}
