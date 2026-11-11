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
/** Adapt actual output items to the saving owner's backend or retained-only source ports. */
import type { CanvasAttachmentOptions } from './CanvasAttachmentOptions.js';
import type { BrowserOutputTransfer } from '../../output_saving/BrowserOutputTransfer.js';
import { retainedOutputCapabilities } from '../../output_saving/BrowserRetainedOutputTransfer.js';
import { retainedCanvasOutput } from './RetainedCanvasOutput.js';

/** Preserve existing backend actions while accurately limiting immutable retained-only originals. */
export function canvasOutputTransfers(
  transfer: Pick<BrowserOutputTransfer, 'copy' | 'fileAction' | 'profile' | 'photoshop' | 'drag'>,
): Pick<
  CanvasAttachmentOptions,
  'copy' | 'fileAction' | 'mediaProfile' | 'mediaCapabilities' | 'photoshop' | 'drag'
> {
  return {
    copy: (item, accepts, format, signal) =>
      transfer.copy(item.file, accepts, format, signal, retainedCanvasOutput(item)),
    fileAction: (item, accepts, signal) =>
      transfer.fileAction(
        item.file,
        accepts,
        signal,
        item.mediaKind === 'image' ? 'image' : 'media',
        retainedCanvasOutput(item),
      ),
    mediaProfile: (item) => transfer.profile(retainedCanvasOutput(item)),
    mediaCapabilities: (item) => {
      if (item.unavailable) return { copy: false, file: false, photoshop: false, drag: false };
      const source = retainedCanvasOutput(item);
      if (source) {
        const actions = retainedOutputCapabilities(source);
        return {
          copy: actions.copy,
          file: actions.download,
          photoshop: actions.photoshop,
          drag: actions.drag,
        };
      }
      return {
        copy: item.mediaKind === 'image',
        file: true,
        photoshop: transfer.profile().photoshop,
        drag: item.mediaKind === 'image',
      };
    },
    photoshop: (document, layers, accepts, signal) => {
      if (layers.some((layer) => layer.item.unavailable)) return Promise.resolve(false);
      return transfer.photoshop(
        document,
        layers.map((layer) => {
          const retained = retainedCanvasOutput(layer.item);
          return { file: layer.item.file, name: layer.name, ...(retained ? { retained } : {}) };
        }),
        accepts,
        signal,
      );
    },
    drag: (item, signal) => {
      if (item.unavailable) return Promise.resolve(null);
      const source = retainedCanvasOutput(item);
      if (source && !retainedOutputCapabilities(source).drag) return Promise.resolve(null);
      return transfer.drag(item.file, signal, source);
    },
  };
}
