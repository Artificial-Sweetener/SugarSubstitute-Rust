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
/** Read Comfy input file sizes through the browser's image decoder. */
import type { CanvasSurfaceFileReference } from '../../../../../integrations/contracts/CanvasSurfaceProtocol.generated.js';
import type { WorkflowFileDimensions } from '../application/WorkflowSurfaceProvisioning.js';

/** Load each file through Comfy's `/view` route on a detached image and keep only its size. */
export class BrowserWorkflowFileDimensions implements WorkflowFileDimensions {
  /** Create detached images in the page that owns the Comfy origin. */
  constructor(private readonly document: Document) {}

  /** Resolve the decoded size, or null when the file cannot be decoded. */
  async read(file: CanvasSurfaceFileReference): Promise<{ width: number; height: number } | null> {
    const image = this.document.createElement('img');
    image.decoding = 'async';
    image.src =
      '/view?' +
      new URLSearchParams({
        filename: file.filename,
        subfolder: file.subfolder,
        type: file.type,
      }).toString();
    const decoded = await image.decode().then(
      () => true,
      // A missing or undecodable file has no verifiable size; callers report null as unverified.
      () => false,
    );
    const size =
      decoded && image.naturalWidth > 0 && image.naturalHeight > 0
        ? { width: image.naturalWidth, height: image.naturalHeight }
        : null;
    image.removeAttribute('src');
    return size;
  }
}
