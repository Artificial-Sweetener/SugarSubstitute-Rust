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
/** A single last-input process memo reuses the actual effective schedule/index while unrelated prompt text changes. */
import type { PromptDocument } from '../../../core/src/index.js';
import type { LoraResource, LoraResourceSnapshot } from './contracts.js';
import type {
  EffectiveLoraSnapshot,
  PreparedLoraSchedulingContext,
} from './scheduled-contracts.js';
import { prepareEffectiveLoras } from './scheduled.js';
import { loraResourceNames } from './resource-names.js';
import { readLoraResources } from './validation.js';

/** Own one derived model/index lifetime; no editable source, caret, history, request or persistent cache is retained. */
export class EffectiveLoraSession {
  private previous: {
    readonly revision: string | null;
    readonly context: PreparedLoraSchedulingContext | null;
    readonly names: readonly string[];
    readonly resources: readonly LoraResource[];
    readonly snapshot: EffectiveLoraSnapshot;
  } | null = null;
  private disposed = false;

  /** Capture current input and retain actual output only when every ordered name, validated descriptor and host identity matches. */
  prepare(
    document: PromptDocument,
    resources: LoraResourceSnapshot,
    revision: string | null,
    context: PreparedLoraSchedulingContext | null,
  ): EffectiveLoraSnapshot {
    return this.prepareNames(loraResourceNames(document), resources, revision, context);
  }

  /** Reuse this same finite memo for parser-prepared scene names, with exact scoped resource coverage. */
  prepareNames(
    names: readonly string[],
    resources: LoraResourceSnapshot,
    revision: string | null,
    context: PreparedLoraSchedulingContext | null,
  ): EffectiveLoraSnapshot {
    if (this.disposed) throw new Error('Effective LoRA session disposed');
    if (
      !context ||
      !context.supported ||
      context.catalogRevision !== revision ||
      resources.status !== 'ready'
    ) {
      this.previous = null;
      return prepareEffectiveLoras(names, resources, revision, context);
    }
    const captured = readLoraResources(resources.resources, names);
    const previous = this.previous;
    if (
      previous &&
      previous.revision === revision &&
      previous.context === context &&
      names.length === previous.names.length &&
      names.every((name, index) => name === previous.names[index]) &&
      captured.every((item, index) => item === previous.resources[index])
    )
      return previous.snapshot;
    const snapshot = prepareEffectiveLoras(
      names,
      { status: 'ready', resources: captured },
      revision,
      context,
    );
    this.previous =
      snapshot.status === 'ready'
        ? { revision, context, names, resources: captured, snapshot }
        : null;
    return snapshot;
  }

  /** Release captured metadata and actual lookup output when the native preparation owner is destroyed. */
  dispose(): void {
    this.disposed = true;
    this.previous = null;
  }
}
