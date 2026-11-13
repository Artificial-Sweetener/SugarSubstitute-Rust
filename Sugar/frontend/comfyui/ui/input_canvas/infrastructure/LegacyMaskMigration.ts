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
/** Preserve saved private masks as actual immutable source files at the public metadata migration boundary. */
import type { CanvasMaskHost } from '../../core/embedded/CanvasRuntime.js';
import type { CanvasMaskReference } from '../../core/embedded/CanvasRuntime.js';
import type { CanvasPixelRaster } from '../../core/embedded/CanvasRuntime.js';
import { maskSourceImage } from '../domain/MaskSourcePixels.js';
import type { InputBinding } from '../domain/InputBinding.js';
import { readInputBinding } from './InputBindingMetadata.js';
import { readLegacyMaskBinding } from './LegacyMaskBinding.js';

/** Keep compatibility capture outside the current source-targeting application model. */
export class LegacyMaskMigration {
  /** Borrow legacy native support only for existing saved references and use the normal source uploader. */
  constructor(
    private readonly host: (id: string) => CanvasMaskHost | null,
    private readonly assets: { stage(pixels: CanvasPixelRaster): Promise<string> },
  ) {}

  /** Convert actual stored coverage without substituting a former filename or the current selection. */
  async read(properties: unknown): Promise<InputBinding | null> {
    const current = readInputBinding(properties, 'mask');
    if (current) return current;
    const legacy = readLegacyMaskBinding(properties);
    if (!legacy) return null;
    const groups = new Map<string, CanvasMaskReference[]>();
    for (const entry of legacy.entries)
      if (entry.mask) {
        const key = JSON.stringify([entry.mask.canvasId, entry.mask.document]);
        const group = groups.get(key) ?? [];
        if (!group.some((mask) => mask.id === entry.mask?.id)) group.push(entry.mask);
        groups.set(key, group);
      }
    const files = new Map<string, string>();
    for (const masks of groups.values()) {
      const first = masks[0];
      if (!first) continue;
      const host = this.host(first.canvasId);
      if (!host)
        throw new Error(
          'Connect the canvas containing the saved mask before loading this workflow.',
        );
      const result = await host.execute({ type: 'capture', masks, maximumSize: 0 });
      if (result.kind === 'failure') throw new Error(result.message);
      if (
        result.kind !== 'captured' ||
        result.snapshots.length !== masks.length ||
        new Set(result.snapshots.map((snapshot) => snapshot.revision)).size > 1
      )
        throw new Error('The saved canvas masks could not be captured coherently.');
      for (const mask of masks) {
        const values = result.snapshots.filter(
            (snapshot) => JSON.stringify(snapshot.mask) === JSON.stringify(mask),
          ),
          snapshot = values[0];
        if (
          values.length !== 1 ||
          !snapshot ||
          snapshot.pixels.width !== mask.document.width ||
          snapshot.pixels.height !== mask.document.height
        )
          throw new Error('The saved canvas mask identity or geometry changed.');
        const image = maskSourceImage({ ...snapshot.pixels, format: 'coverage8' });
        files.set(JSON.stringify(mask), await this.assets.stage(image));
      }
    }
    return {
      version: 2,
      field: legacy.field,
      files: legacy.files,
      selected: legacy.selected,
      entries: legacy.entries.map((entry) => ({
        id: entry.id,
        label: entry.label,
        source: {
          kind: 'file',
          file: entry.mask ? (files.get(JSON.stringify(entry.mask)) ?? null) : entry.file,
        },
      })),
    };
  }
}
