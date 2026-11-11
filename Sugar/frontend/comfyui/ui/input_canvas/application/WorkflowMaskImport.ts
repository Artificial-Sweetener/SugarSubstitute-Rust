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
/** Import pictures as new regions of a regional mask loader on a workflow-surface canvas. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { InputSourceAssets } from './InputSourcePreparation.js';
import { maskSourceImage } from '../domain/MaskSourcePixels.js';
import type { WorkflowSurfaceSource } from './WorkflowInputRoute.js';

/** Stretch a picture to a surface's exact size and read its gray levels as coverage. */
export interface MaskImportRasters {
  raster(file: Blob, width: number, height: number): Promise<CanvasPixelRaster>;
}

/**
 * Convert every picture to the loader's surface size before anything is added, so a failed
 * picture adds no region. Files are staged in the coverage image encoding (rgb = value,
 * alpha = 255 - value), which every loader channel reads as the same coverage. Each staged file
 * becomes a new region's initial mask; the surface
 * provisioning then creates and binds its layer.
 */
export class WorkflowMaskImport {
  /** Share the staging owner used for queue-time captures. */
  constructor(
    private readonly surfaces: WorkflowSurfaceSource,
    private readonly rasters: MaskImportRasters,
    private readonly assets: InputSourceAssets,
  ) {}

  /** Whether the loader's masks live on a surface that can receive imports. */
  available(endpoint: string): boolean {
    return this.#surface(endpoint) !== null;
  }

  /** Stage each picture and hand the staged files to the loader in order. */
  async import(
    endpoint: string,
    files: readonly Blob[],
    append: (staged: readonly string[]) => void,
  ): Promise<void> {
    const surface = this.#surface(endpoint);
    if (!surface) throw new Error('The mask has no input canvas.');
    const staged: string[] = [];
    for (const file of files)
      staged.push(
        await this.assets.stage(
          maskSourceImage(
            await this.rasters.raster(file, surface.document.width, surface.document.height),
          ),
        ),
      );
    if (staged.length) append(staged);
  }

  /** Find the surface holding the loader's masks. */
  #surface(endpoint: string) {
    return (
      this.surfaces.state?.surfaces.find((surface) =>
        surface.masks.some((mask) => mask.endpoint === endpoint),
      ) ?? null
    );
  }
}
