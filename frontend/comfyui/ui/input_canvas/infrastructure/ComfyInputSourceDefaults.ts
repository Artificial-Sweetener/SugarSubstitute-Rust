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
/** Resolve saved mask sources and default unconfigured loaders through the attached canvas capability. */
import type { CanvasSourceHost } from '../../core/embedded/CanvasRuntime.js';
import type { InputBinding } from '../domain/InputBinding.js';
import type { InputLoader } from '../domain/InputLoader.js';
import { readInputBinding } from './InputBindingMetadata.js';
import { inputLabel } from '../domain/InputLoader.js';
import { defaultCanvasInput } from '../domain/CanvasInputSource.js';
import type { LegacyMaskMigration } from './LegacyMaskMigration.js';

/** Use the same default policy for loaders queued before masonry has mounted their controls. */
export class ComfyInputSourceDefaults {
  /** Resolve native capabilities and existing persisted values at their adapter boundary. */
  constructor(
    private readonly host: () => CanvasSourceHost | null,
    private readonly legacy: Pick<LegacyMaskMigration, 'read'>,
    private readonly provisions: (canvasId: string) => boolean,
  ) {}
  /**
   * Preserve explicit choices and prefer current selection only for unconfigured capable canvases.
   * A canvas that provisions workflow surfaces keeps unconfigured loaders on their files.
   */
  async resolve(
    properties: unknown,
    loader: InputLoader,
    files: readonly string[],
  ): Promise<InputBinding | null> {
    const saved =
      readInputBinding(properties, loader.kind, loader.field) ??
      (loader.kind === 'mask' ? await this.legacy.read(properties) : null);
    if (saved) return saved;
    const canvas = this.host();
    if (!canvas || this.provisions(canvas.canvasId)) return null;
    const context = await canvas.execute({ type: 'context' });
    if (context.kind === 'failure') throw new Error(context.message);
    if (
      context.kind !== 'context' ||
      !context.supported ||
      !context.targets.includes(loader.kind === 'mask' ? 'selection' : 'composition')
    )
      return null;
    const entries = (loader.ordered && files.length ? files : [files[0] ?? null]).map(
      (file, index) => ({
        id: `source-${String(index + 1)}`,
        label: `${inputLabel(loader.kind)} ${String(index + 1)}`,
        source: defaultCanvasInput(
          canvas.canvasId,
          context,
          file,
          loader.kind === 'mask' ? 'selection' : 'composition',
        ),
      }),
    );
    return { version: 2, field: loader.field, files, entries, selected: entries[0]?.id ?? null };
  }
}
