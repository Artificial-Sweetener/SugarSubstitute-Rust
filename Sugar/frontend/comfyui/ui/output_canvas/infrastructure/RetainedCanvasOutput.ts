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
/** Pass exact retained-only backing to the existing saving owner without replacing backend identity. */
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { BrowserRetainedOutputSource } from '../../output_saving/BrowserRetainedOutputTransfer.js';

/** Backend-resolved originals retain existing actions; only a verified retained presentation carries this source. */
export function retainedCanvasOutput(
  item: WorkflowOutputItem | undefined,
): BrowserRetainedOutputSource | undefined {
  if (!item?.presentationUrl) return undefined;
  const artifact = item.artifact;
  if (
    !artifact ||
    artifact.output.id !== item.id ||
    artifact.binding.runtimeSession !== item.provenance.sessionId ||
    artifact.binding.generation !== item.provenance.generation ||
    artifact.output.entryId !== artifact.binding.entryId ||
    artifact.output.assetDigest !== artifact.output.digest
  )
    throw new TypeError('Retained canvas output does not match its admitted immutable artifact.');
  return { output: artifact.output, binding: artifact.binding, url: item.presentationUrl };
}
