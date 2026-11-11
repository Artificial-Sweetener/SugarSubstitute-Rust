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
/** Coordinate SugarCubes graph-instance discovery and projection. */

import { allocateGraphInstanceAliases } from './AliasAllocator.js';
import { getGraphGroups } from './GraphQuery.js';
import { getGroupSugarcubes, ensureGroupSerialization } from './GroupMetadata.js';
import { InstanceBuilder } from './InstanceBuilder.js';
import { InstanceGroupPresenter } from './InstanceGroupPresenter.js';
import { InstanceGroupReconciler } from './InstanceGroupReconciler.js';
import type {
  InstanceAdapter,
  InstanceBuilderTarget,
  InstanceEvents,
  InstanceManagerOptions,
  InstanceRefreshOptions,
  InstanceScheduler,
  ManagedGroup,
} from './InstanceManagerContracts.js';

export type { InstanceAdapter } from './InstanceManagerContracts.js';

/** Coordinate instance refresh scheduling, reconciliation, and presentation. */
export class InstanceManager {
  private readonly adapter: InstanceAdapter;
  private readonly events: InstanceEvents | null;
  private readonly scheduler: InstanceScheduler | null;
  readonly instanceBuilder: InstanceBuilderTarget;
  private readonly reconciler: InstanceGroupReconciler;
  private readonly presenter: InstanceGroupPresenter;
  private scheduled = false;
  private pendingForce = false;
  private lastSignature: string | null = null;

  constructor({
    adapter,
    events = null,
    scheduler = null,
    instanceBuilder = null,
    requestDirtyRefresh = null,
  }: InstanceManagerOptions) {
    this.adapter = adapter;
    this.events = events;
    this.scheduler = scheduler;
    this.instanceBuilder =
      instanceBuilder || new InstanceBuilder({ logger: adapter.getConsole?.() ?? null });
    this.reconciler = new InstanceGroupReconciler();
    this.presenter = new InstanceGroupPresenter({
      adapter,
      events,
      requestDirtyRefresh: typeof requestDirtyRefresh === 'function' ? requestDirtyRefresh : null,
    });
  }

  /** Install group serialization support required by managed instances. */
  setup(): void {
    ensureGroupSerialization(this.adapter);
  }

  /** Schedule one coalesced graph-instance refresh. */
  scheduleRefresh({ graph, reason, force = false }: InstanceRefreshOptions = {}): void {
    if (force) this.pendingForce = true;
    if (this.scheduled) return;
    this.scheduled = true;
    this.scheduler?.raf?.(() => {
      const shouldForce = this.pendingForce;
      this.scheduled = false;
      this.pendingForce = false;
      this.refresh({ graph, reason, force: shouldForce });
    });
  }

  /** Reconcile discovered Cube instances with their managed graph groups. */
  refresh({ graph, force = false }: InstanceRefreshOptions = {}): void {
    if (!graph || !this.adapter.getLiteGraph?.()?.LGraphGroup) return;

    const groups = getGraphGroups(graph) as ManagedGroup[];
    const instances = this.instanceBuilder.build(graph);
    const signature = this.reconciler.buildSignature(instances, groups);
    if (!force && this.lastSignature === signature) return;
    this.lastSignature = signature;

    const matches = this.reconciler.resolve(instances, groups);
    const aliases = allocateGraphInstanceAliases(matches);
    const appliedGroups = new Set<ManagedGroup>();
    for (const { instance, group } of matches) {
      const applied = this.presenter.apply(
        instance,
        group,
        graph,
        aliases.get(instance.instanceId),
      );
      if (applied) appliedGroups.add(applied);
    }

    for (const group of groups) {
      if (getGroupSugarcubes(group)?.managed && !appliedGroups.has(group)) {
        graph.remove?.(group);
      }
    }

    this.events?.emit?.('cube:instances:updated', {
      graph,
      instances: matches.map((match) => match.instance),
    });
  }
}
