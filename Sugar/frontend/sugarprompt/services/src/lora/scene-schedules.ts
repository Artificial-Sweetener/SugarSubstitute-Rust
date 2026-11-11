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
/** One finite session prepares actual model/action output before exact-position lookup. */
import { requireValue } from '../../../core/src/required.js';
import type { LoraResourceSnapshot } from './contracts.js';
import type {
  EffectiveLoraSnapshot,
  PreparedLoraSchedulingContext,
  ScheduledLora,
} from './scheduled-contracts.js';
import type {
  PreparedSceneLoraScopes,
  PreparedDocumentLoraScopes,
  DocumentLoraScopeEntry,
} from './scene-scopes.js';
import { EffectiveLoraSession } from './scheduled-session.js';
import { readLoraResources } from './validation.js';
import { prepareLoraTriggerActions } from './trigger-actions.js';
/** Prepared actions share the actual effective snapshot and canonical model identity. */
export interface SceneLoraSchedule {
  readonly snapshot: EffectiveLoraSnapshot;
  readonly insertions: ReadonlyMap<string, ScheduledLora>;
}
/** Public prompt lookup retains its exact-source UTF-16 contract and prompt scene behavior. */
export interface PreparedSceneLoraSchedules {
  /** Look up actual output prepared before input; invalid prompt boundaries reject. */
  at(position: number): SceneLoraSchedule;
  /** Constant prepared eligibility follows exact-source UTF-16 and an optional canonical backend key. */
  canInsertAt(position: number, backendKey?: string): boolean;
}
/** Structured storage has no scheduling authority outside admitted decoded values. */
export interface PreparedDocumentLoraSchedules {
  /** Exact-source UTF-16 lookup returns unavailable for headers, padding, malformed input and unsafe endpoints. */
  at(position: number): SceneLoraSchedule | null;
  /** Unavailable scopes cannot insert triggers. */
  canInsertAt(position: number, backendKey?: string): boolean;
}
/** One shared input contract adapts prompt scenes and canonical document values without another schedule algorithm. */
interface LoraScopeLookup {
  readonly entries: readonly DocumentLoraScopeEntry[];
  readonly resourceNames: readonly string[];
  /** Lookup consumes already prepared names at exact-source UTF-16 boundaries. */
  namesAt(position: number): readonly string[] | null;
}
/** One finite 64-entry ordered-name memo owns every effective session and issued facade's disposal fence. */
export class SceneLoraSession {
  private readonly cache = new Map<
    string,
    {
      readonly session: EffectiveLoraSession;
      result: SceneLoraSchedule | null;
    }
  >();
  private disposed = false;
  /** Preserve the public prompt adapter while delegating actual preparation to the shared finite owner. */
  prepare(
    scopes: PreparedSceneLoraScopes,
    resources: LoraResourceSnapshot,
    revision: string | null,
    context: PreparedLoraSchedulingContext | null,
  ): PreparedSceneLoraSchedules {
    const prepared = this.prepareEntries(scopes, resources, revision, context);
    return Object.freeze({
      /** Prompt namesAt validates boundaries; an absent admitted prompt entry is an invariant violation. */
      at(position: number): SceneLoraSchedule {
        const schedule = prepared.at(position);
        if (!schedule) throw new Error('missing-prompt-lora-schedule');
        return schedule;
      },
      canInsertAt: prepared.canInsertAt,
    });
  }
  /** Prepare every admitted canonical value/scene entry under this same session and metadata lifetime. */
  prepareDocument(
    scopes: PreparedDocumentLoraScopes,
    resources: LoraResourceSnapshot,
    revision: string | null,
    context: PreparedLoraSchedulingContext | null,
  ): PreparedDocumentLoraSchedules {
    return this.prepareEntries(scopes, resources, revision, context);
  }
  /** Eagerly build each distinct ordered-name result; hot facades perform only scope and actual-object lookups. */
  private prepareEntries(
    scopes: LoraScopeLookup,
    resources: LoraResourceSnapshot,
    revision: string | null,
    context: PreparedLoraSchedulingContext | null,
  ): PreparedDocumentLoraSchedules {
    if (this.disposed) throw new Error('Scene LoRA session disposed');
    const status = resources.status;
    const pool = new Map(
      (status === 'ready' ? readLoraResources(resources.resources, scopes.resourceNames) : []).map(
        (item) => [item.name, item],
      ),
    );
    const results = new Map<readonly string[], SceneLoraSchedule>();
    const distinct = new Map<string, SceneLoraSchedule>();
    for (const { names } of scopes.entries) {
      const key = JSON.stringify(names);
      let result = distinct.get(key);
      if (!result) {
        let retained = this.cache.get(key);
        if (!retained) retained = { session: new EffectiveLoraSession(), result: null };
        const snapshot = retained.session.prepareNames(
          names,
          {
            status,
            resources:
              status === 'ready'
                ? names.map((name) => requireValue(pool.get(name), 'scene-schedules'))
                : [],
          },
          revision,
          context,
        );
        result =
          retained.result?.snapshot === snapshot
            ? retained.result
            : Object.freeze({
                snapshot,
                insertions: prepareLoraTriggerActions(snapshot),
              });
        retained.result = result;
        this.cache.delete(key);
        this.cache.set(key, retained);
        if (this.cache.size > 64) {
          const oldest = requireValue(this.cache.keys().next().value, 'scene-schedules');
          requireValue(this.cache.get(oldest), 'scene-schedules').session.dispose();
          this.cache.delete(oldest);
        }
        distinct.set(key, result);
      }
      results.set(names, result);
    }
    /** Every namesAt result must be an actual prepared entry; copied or omitted names are invariant failures. */
    const at = (position: number): SceneLoraSchedule | null => {
      if (this.disposed) throw new Error('Scene LoRA session disposed');
      const names = scopes.namesAt(position);
      if (names === null) return null;
      const result = results.get(names);
      if (!result) throw new Error('missing-prepared-lora-schedule');
      return result;
    };
    const result: PreparedDocumentLoraSchedules = Object.freeze({
      at,
      /** Trigger indexes are actual snapshot-derived preparation output, never rebuilt during input. */
      canInsertAt(position: number, backendKey?: string): boolean {
        const schedule = at(position);
        return (
          schedule !== null &&
          (backendKey === undefined
            ? schedule.insertions.size > 0
            : schedule.insertions.has(backendKey))
        );
      },
    });
    return result;
  }
  /** Dispose each retained effective session and reject all issued facades, including already evicted entries. */
  dispose(): void {
    this.disposed = true;
    for (const entry of this.cache.values()) entry.session.dispose();
    this.cache.clear();
  }
}
