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
/** Adapt outgoing workflows to the single shared full-resolution source capture use case. */
import {
  preparedSourceFile,
  type InputSourcePreparation,
} from '../application/InputSourcePreparation.js';
import type { CanvasSourceEntry } from '../application/CanvasSourceCapture.js';
import type { ComfyInputSourceDefaults } from './ComfyInputSourceDefaults.js';
import type { ComfyInputLoaders } from './ComfyInputLoaders.js';
import { inputWorkflowProjection } from './InputWorkflowProjection.js';
import type { ComfyInputReturnPreparation } from './ComfyInputReturnPreparation.js';
import type { WorkflowPromptPreparation } from '../../execution/domain/WorkflowPreparation.js';
import type { ComfyMaskInputPreparation } from './ComfyMaskInputPreparation.js';
import type { PreparedInputPromptClient } from './PreparedInputPromptClient.js';
import { isRecord } from '../../types/common.js';

/** Prepare detached execution inputs and immutable return placement for both queue routes. */
export class ComfyInputPreparation {
  /** Share the exact catalog and capture owner used by masonry bindings. */
  constructor(
    private readonly loaders: ComfyInputLoaders,
    private readonly sources: InputSourcePreparation,
    private readonly defaults: ComfyInputSourceDefaults,
    private readonly returns: ComfyInputReturnPreparation,
    private readonly maskInputs: ComfyMaskInputPreparation,
    private readonly promptInputs: Pick<PreparedInputPromptClient, 'prepare'>,
  ) {}
  /** Pin entry identity, order and channel before any asynchronous native capture or upload. */
  async prepare(payload: unknown): Promise<WorkflowPromptPreparation> {
    const pairs = this.maskInputs.pairs();
    const projection = await inputWorkflowProjection(
      payload,
      this.loaders,
      (properties, loader, files) => this.defaults.resolve(properties, loader, files),
    );
    const entries: CanvasSourceEntry[] = projection.targets.flatMap((target) =>
      target.binding.entries.flatMap((entry) =>
        entry.source.kind === 'canvas'
          ? [
              {
                key: JSON.stringify([target.key, entry.id]),
                source: entry.source,
                representation:
                  target.loader.kind === 'image' ? ('image' as const) : ('raw' as const),
              },
            ]
          : [],
      ),
    );
    if (!projection.targets.length) return { payload };
    const ports = this.loaders.assetPorts();
    const origin = await this.returns.origin(projection.targets);
    const sources = await this.sources.prepare(entries);
    const masks = await this.maskInputs.prepare(
      projection.targets,
      sources,
      pairs,
      ports,
      origin?.document ?? null,
    );
    const canvas = await this.returns.prepare(
      projection.targets,
      sources,
      ports,
      origin,
      masks.returnInputs,
    );
    for (const target of projection.targets)
      target.apply(
        target.binding.entries.map((entry) => {
          return (
            masks.sourceFiles.get(target.key) ?? preparedSourceFile(target.key, entry, sources)
          );
        }),
      );
    const prepared = projection.payload;
    if (masks.replacements.length && isRecord(prepared) && isRecord(prepared.output))
      prepared.output = await this.promptInputs.prepare(prepared.output, masks.replacements);
    return {
      payload: prepared,
      ...(canvas ? { canvas } : {}),
      ...(masks.replacements.length && (!isRecord(prepared) || !isRecord(prepared.output))
        ? { preparedInputs: masks.replacements }
        : {}),
    };
  }
}
