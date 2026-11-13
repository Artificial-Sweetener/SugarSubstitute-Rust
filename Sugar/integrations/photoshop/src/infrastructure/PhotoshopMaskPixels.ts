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
/** Read and restore exact selection coverage without using bounds as a substitute for a mask. */
import { imaging } from 'photoshop';
import type { PhotoshopDocument } from './PhotoshopCanvasIdentity.js';
import type { CanvasMaskPixels } from '../../../contracts/CanvasMasksProtocol.generated.js';
import { decodeMaskPixels } from '../../../shared/CanvasMaskPixels.js';

/** Restore one full-resolution coverage image through Photoshop's native selection API. */
export async function putSelectionPixels(
  document: PhotoshopDocument,
  pixels: CanvasMaskPixels,
): Promise<void> {
  if (pixels.width !== document.width || pixels.height !== document.height)
    throw new Error('The mask and Photoshop document dimensions differ.');
  const image = await imaging.createImageDataFromBuffer(decodeMaskPixels(pixels), {
    width: pixels.width,
    height: pixels.height,
    components: 1,
    colorSpace: 'Grayscale',
  });
  try {
    await imaging.putSelection({
      documentID: document.id,
      imageData: image,
      replace: true,
      targetBounds: { left: 0, top: 0, width: pixels.width, height: pixels.height },
    });
  } finally {
    await image.dispose();
  }
}
