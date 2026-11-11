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
/** One catalog lifetime owns bounded metadata memoization and latest-result preparation, independent of frameworks. */
import { requireValue } from '../../../core/src/required.js';
import type { PromptDocument } from '../../../core/src/index.js';
import { LatestRequest } from '../requests/latest.js';
import type {
  LoraResource,
  LoraResourceAdapter,
  LoraResourceSnapshot,
  LoraWarmupOutcome,
} from './contracts.js';
import { readLoraResources } from './validation.js';
import { loraResourceNames } from './resource-names.js';
import { isCatalogStableLoraResource } from './resource-validity.js';
/** Prepare parsed LoRA resources; source changes retire publication while catalog-valid cached output remains reusable. */
export class LoraResourceSession {
  private readonly lane = new LatestRequest();
  private readonly cache = new Map<string, LoraResource>();
  private readonly adapter: LoraResourceAdapter | undefined;
  private readonly revision: string | undefined;
  private readonly observe: (snapshot: LoraResourceSnapshot) => void;
  private readonly capacity: number;
  private current: LoraResourceSnapshot;
  private epoch = 0;
  private disposed = false;
  /** Bind one immutable catalog and finite observer; capacity bounds process memoization, never persisted user data. */
  constructor(
    adapter: LoraResourceAdapter | undefined,
    observe: (snapshot: LoraResourceSnapshot) => void = () => {},
    capacity = 64,
  ) {
    if (!Number.isSafeInteger(capacity) || capacity < 1 || capacity > 1024)
      throw new RangeError('Invalid LoRA cache capacity');
    if (
      adapter &&
      (typeof adapter.revision !== 'string' ||
        !adapter.revision.length ||
        adapter.revision.length > 256)
    )
      throw new TypeError('Invalid LoRA catalog revision');
    this.adapter = adapter;
    this.revision = adapter?.revision;
    this.observe = observe;
    this.capacity = capacity;
    this.current = Object.freeze({
      status: adapter ? 'idle' : 'unavailable',
      resources: Object.freeze([]),
    });
  }
  /** Expose immutable external preparation, without retaining an editable source document. */
  snapshot(): LoraResourceSnapshot {
    return this.current;
  }
  /** Prepare unique exact names from parsed source, reuse actual validated records and supersede older host work. */
  async warmup(document: PromptDocument): Promise<LoraWarmupOutcome> {
    return this.warmupNames(loraResourceNames(document));
  }
  /** Resolve exact parser-prepared names across the source and its effective scenes through the same finite catalog lifetime. */
  async warmupNames(names: readonly string[]): Promise<LoraWarmupOutcome> {
    if (names.some((name) => typeof name !== 'string') || new Set(names).size !== names.length)
      throw new TypeError('Invalid LoRA resource names');
    names = Object.freeze([...names]);
    if (this.disposed) return { status: 'disposed' };
    if (!this.adapter) return { status: 'unavailable' };
    if (this.adapter.revision !== this.revision)
      throw new Error('LoRA catalog mutated without replacement');
    const epoch = ++this.epoch;
    this.lane.cancel();
    if (this.disposed) return { status: 'disposed' };
    if (epoch !== this.epoch) return { status: 'cancelled' };
    const reused = new Map<string, LoraResource>();
    const retained = new Map(
      this.current.resources
        .filter(isCatalogStableLoraResource)
        .map((resource) => [resource.name, resource]),
    );
    for (const name of names) {
      const cached = this.cache.get(name);
      const value = cached ?? retained.get(name);
      if (value) {
        if (cached) {
          this.cache.delete(name);
          this.cache.set(name, cached);
        }
        reused.set(name, value);
      }
    }
    const missing = Object.freeze(names.filter((name) => !reused.has(name)));
    try {
      if (missing.length) {
        this.publish('loading', [...reused.values()]);
        if (this.disposed) return { status: 'disposed' };
        if (epoch !== this.epoch) return { status: 'cancelled' };
        const result = await this.lane.run((signal) =>
          requireValue(this.adapter, 'session').resolve(missing, signal),
        );
        if (result.status !== 'ready') return result;
        if (this.disposed) return { status: 'disposed' };
        if (epoch !== this.epoch) return { status: 'cancelled' };
        if (this.adapter.revision !== this.revision)
          throw new Error('LoRA catalog mutated during request');
        for (const value of readLoraResources(result.value, missing)) {
          reused.set(value.name, value);
          // Transient authority/service states cannot become cached absence or collision decisions.
          if (isCatalogStableLoraResource(value)) {
            this.cache.set(value.name, value);
            while (this.cache.size > this.capacity)
              this.cache.delete(requireValue(this.cache.keys().next().value, 'session'));
          }
        }
      }
      const resources = Object.freeze(
        names.map((name) => requireValue(reused.get(name), 'session')),
      );
      this.publish('ready', resources);
      if (this.disposed) return { status: 'disposed' };
      if (epoch !== this.epoch) return { status: 'cancelled' };
      return { status: 'ready', resources };
    } catch (cause) {
      if (this.disposed) return { status: 'disposed' };
      if (epoch !== this.epoch) return { status: 'cancelled' };
      this.publish('failed', [...reused.values()]);
      throw cause;
    }
  }
  /** Settle retired source/profile work while retaining published, catalog-valid records; replacing the session retires catalog authority. */
  invalidate(): void {
    if (this.disposed) return;
    const epoch = ++this.epoch;
    this.lane.cancel();
    if (this.disposed || epoch !== this.epoch) return;
    this.publish(
      this.adapter ? 'idle' : 'unavailable',
      this.current.resources.filter(isCatalogStableLoraResource),
    );
  }
  /** Release cached metadata, settle ignored-abort work and prohibit publication after teardown. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    ++this.epoch;
    this.lane.dispose();
    this.cache.clear();
    this.current = Object.freeze({
      status: 'disposed',
      resources: Object.freeze([]),
    });
  }
  /** Only this finite owner publishes immutable external records; no source is included in lifecycle diagnostics. */
  private publish(
    status: LoraResourceSnapshot['status'],
    resources: readonly LoraResource[],
  ): void {
    if (this.disposed) return;
    if (
      this.current.status === status &&
      this.current.resources.length === resources.length &&
      resources.every((value, index) => this.current.resources[index] === value)
    )
      return;
    this.current = Object.freeze({
      status,
      resources: Object.freeze(resources),
    });
    this.observe(this.current);
  }
}
