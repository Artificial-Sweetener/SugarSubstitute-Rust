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
/** One native media lifetime retains current rasters plus bounded inactive memoization and schedules finite decoding. */
import { requireValue } from '../../core/src/required.js';
import { LoraMediaJob } from './lora-media-job.js';
import {
  pendingLoraMedia,
  unavailableLoraMedia,
  type LoraMediaDecoder,
  type LoraMediaReference,
  type PreparedLoraMedia,
} from './lora-media-contracts.js';
/** A descriptor has one decode/raster owner; current membership pins it until a new prepared snapshot is installed. */
interface MediaEntry {
  value: PreparedLoraMedia;
  job: LoraMediaJob | null;
}
/** Only inactive prepared rasters are memoized; current unique descriptors are source-derived rendering input, not a second catalog. */
export class LoraMediaPool {
  private readonly decode: LoraMediaDecoder;
  private readonly capacity: number;
  private readonly concurrency: number;
  private readonly entries = new Map<LoraMediaReference, MediaEntry>();
  private current = new Set<LoraMediaReference>();
  private observe: (() => void) | undefined;
  private report: ((cause: unknown) => void) | undefined;
  private running = 0;
  private disposed = false;
  private paused = false;
  private pumping = false;
  /** Bind one view lifetime to a browser decoder; retained inactive rasters and concurrent work are bounded independently. */
  constructor(
    decode: LoraMediaDecoder,
    observe: () => void,
    report: (cause: unknown) => void,
    capacity = 64,
    concurrency = 4,
  ) {
    if (
      !Number.isSafeInteger(capacity) ||
      capacity < 0 ||
      capacity > 1024 ||
      !Number.isSafeInteger(concurrency) ||
      concurrency < 1 ||
      concurrency > 16
    )
      throw new RangeError('Invalid LoRA media bounds');
    this.observe = observe;
    this.report = report;
    this.decode = decode;
    this.capacity = capacity;
    this.concurrency = concurrency;
  }
  /** Cancel removed work immediately, retain reusable prepared pixels and begin only current descriptor jobs. */
  select(references: readonly LoraMediaReference[]): void {
    if (this.disposed) return;
    this.paused = false;
    this.current = new Set(references);
    for (const [reference, entry] of this.entries) {
      if (!this.current.has(reference) && entry.job) {
        entry.job.cancel();
        entry.job = null;
        this.running--;
        this.entries.delete(reference);
      }
    }
    for (const reference of this.current) {
      let entry = this.entries.get(reference);
      if (!entry) entry = { value: pendingLoraMedia, job: null };
      this.entries.delete(reference);
      this.entries.set(reference, entry);
    }
    this.trim();
    this.pump();
  }
  /** Publish the actual prepared records for only current descriptors; no URL or source enters diagnostics. */
  snapshot(): ReadonlyMap<LoraMediaReference, PreparedLoraMedia> {
    return new Map(
      [...this.current].map((reference) => [
        reference,
        requireValue(this.entries.get(reference), 'lora-media-pool').value,
      ]),
    );
  }
  /** Retire in-flight work while old native widgets still retain their prepared rasters until replacement. */
  pause(): void {
    this.paused = true;
    for (const entry of this.entries.values()) {
      if (entry.job) {
        entry.job.cancel();
        entry.job = null;
        this.running--;
      }
    }
  }
  /** Retire retained state before every attempted release; failures remain observable without skipping later resources. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.observe = undefined;
    this.report = undefined;
    const retained = [...this.entries.values()];
    this.entries.clear();
    this.current.clear();
    this.running = 0;
    const causes: unknown[] = [];
    for (const entry of retained) {
      try {
        this.release(entry);
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length > 1) throw new AggregateError(causes, 'LoRA media retirement failed');
  }
  /** Four concurrent decodes keep acquisition/CPU work outside input/paint; synchronous reentrancy sees the installed job. */
  private pump(): void {
    if (this.pumping) return;
    this.pumping = true;
    try {
      let current: Set<LoraMediaReference>;
      do {
        current = this.current;
        for (const reference of current) {
          if (current !== this.current) break;
          if (this.disposed || this.paused || this.running >= this.concurrency) return;
          const entry = requireValue(this.entries.get(reference), 'lora-media-pool');
          if (entry.value.status !== 'pending' || entry.job) continue;
          const job = new LoraMediaJob((outcome) => {
            if (entry.job !== job || this.disposed) {
              if (outcome.status === 'ready') outcome.raster?.dispose();
              return;
            }
            entry.job = null;
            this.running--;
            entry.value =
              outcome.status === 'ready' && outcome.raster
                ? Object.freeze({ status: 'ready', raster: outcome.raster })
                : unavailableLoraMedia;
            if (outcome.status === 'failed') this.report?.(outcome.cause);
            this.observe?.();
            this.pump();
          });
          entry.job = job;
          this.running++;
          job.start(reference, this.decode);
        }
      } while (current !== this.current);
    } finally {
      this.pumping = false;
    }
  }
  /** Limit inactive process memoization without evicting source-current rasters used by native widgets. */
  private trim(): void {
    for (const [reference, entry] of this.entries)
      if (!this.current.has(reference) && entry.value.status === 'pending')
        this.entries.delete(reference);
    let inactive = this.entries.size - this.current.size;
    for (const [reference, entry] of this.entries) {
      if (inactive <= this.capacity) break;
      if (this.current.has(reference)) continue;
      this.entries.delete(reference);
      this.release(entry);
      inactive--;
    }
  }
  /** Each owned raster closes once; retired jobs independently dispose late returned pixels. */
  private release(entry: MediaEntry): void {
    entry.job?.cancel();
    entry.job = null;
    if (entry.value.status === 'ready') entry.value.raster.dispose();
  }
}
