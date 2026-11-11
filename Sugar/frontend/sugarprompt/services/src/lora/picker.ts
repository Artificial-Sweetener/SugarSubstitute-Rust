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
/** A finite host picker lane validates canonical selections without owning a picker UI or editor document. */
import { loraScheduleText } from '../../../core/src/index.js';
import { LatestRequest } from '../requests/latest.js';
import type { LoraModelIdentity } from './contracts.js';
import { loraRecord, loraText, readLoraModelIdentity } from './metadata-validation.js';

/** The host returns its deliberate prompt identity, independent of the model's visible title. */
export type LoraPickerResult =
  | {
      readonly status: 'selected';
      readonly model: LoraModelIdentity;
      readonly weightText?: string | null;
    }
  | { readonly status: 'cancelled' | 'unavailable' };

/** Hosts open and own their picker UI; abort must close or retire it before publishing selection. */
export interface LoraPickerAdapter {
  pick(signal: AbortSignal): Promise<LoraPickerResult>;
}

/** Prepared selection contains scheduler-safe source; retirement and host unavailability remain explicit. */
export type LoraPickerOutcome =
  | { readonly status: 'selected'; readonly text: string }
  | { readonly status: 'cancelled' | 'disposed' | 'unavailable' };

/** Adapt unknown bridge data with a closed schema; malformed records and unexpected host failures remain observable. */
function selection(value: unknown): LoraPickerOutcome {
  const selected =
    value !== null && typeof value === 'object' && 'status' in value && value.status === 'selected';
  const hasWeight =
    value !== null && typeof value === 'object' && Object.hasOwn(value, 'weightText');
  const raw = loraRecord(
    value,
    selected ? ['status', 'model', ...(hasWeight ? ['weightText'] : [])] : ['status'],
  );
  if (raw.status === 'cancelled' || raw.status === 'unavailable')
    return Object.freeze({ status: raw.status });
  if (raw.status !== 'selected') throw new TypeError('Invalid LoRA picker status');
  const model = readLoraModelIdentity(raw.model);
  const weight =
    raw.weightText === null || raw.weightText === undefined
      ? null
      : loraText(raw.weightText, 4096, true);
  return Object.freeze({
    status: 'selected',
    text: loraScheduleText(model.promptName, weight),
  });
}

/** One external request owner settles source/profile/adapter retirement even when a host ignores its abort signal. */
export class LoraPickerSession {
  private readonly requests = new LatestRequest();
  private disposed = false;
  private adapter: LoraPickerAdapter | undefined;

  /** A component adapter replacement creates a new complete host lifetime. */
  constructor(adapter: LoraPickerAdapter | undefined) {
    this.adapter = adapter;
  }

  /** Superseding picker requests cancel their predecessor; a missing host never manufactures a model selection. */
  async pick(): Promise<LoraPickerOutcome> {
    if (this.disposed) return { status: 'disposed' };
    const adapter = this.adapter;
    if (!adapter) return { status: 'unavailable' };
    const outcome = await this.requests.run((signal) => adapter.pick(signal));
    return outcome.status === 'ready' ? selection(outcome.value) : outcome;
  }

  /** Source, composition and capability changes retire pending selection without retaining a document snapshot. */
  cancel(): void {
    this.requests.cancel();
  }

  /** Unmount/public destroy immediately settle owned work and prevent subsequent host invocation. */
  dispose(): void {
    this.disposed = true;
    this.adapter = undefined;
    this.requests.dispose();
  }
}
