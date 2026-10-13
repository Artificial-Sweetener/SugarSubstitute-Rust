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
/** Bind submitted seed history to native and shared owners without storing widget references in history. */
import type { UnknownRecord } from '../../types/common.js';
import type { ComfyApplication } from '../../types/graph.js';
import type { WorkflowSessionStore } from '../../execution/application/WorkflowSessionStore.js';
import type { WorkflowSeedHistory } from '../../execution/application/WorkflowSeedHistory.js';
import type { SeedInput, SeedReuseControl } from '../../execution/domain/WorkflowSeed.js';
import type { WorkflowRelations } from '../application/WorkflowRelations.js';
import type { NativeRelationInventory } from './NativeRelationInventory.js';
import { nativeSeedPath, restoreNativeSeed } from './NativeSeedReuseBinding.js';
import { linkedFieldMembers } from '../domain/RelationResolver.js';
import { resolveNativeSeedOwner } from '../../surface/appearance/ComfyNativeSeedControl.js';

/** Share one restore action across cards, masonry and global override widget mounts. */
export class NativeWorkflowSeedReuse {
  /** Inject the workflow and relation authorities rather than consulting global browser state. */
  constructor(
    private readonly app: ComfyApplication | null,
    private readonly sessions: WorkflowSessionStore,
    private readonly history: WorkflowSeedHistory,
    private readonly relations: WorkflowRelations,
    private readonly inventory: NativeRelationInventory,
    private readonly document: Document,
    private readonly reportError: (error: unknown) => void,
  ) {}

  /** Resolve a field through its current shared owner, retaining exact node identity after undo. */
  field(node: UnknownRecord, name: string): SeedReuseControl | null {
    const context = this.sessions.read().active;
    const native = resolveNativeSeedOwner(node, name);
    if (!context || !native) return null;
    const current = (): boolean => {
      const active = this.sessions.read().active;
      return (
        active?.sessionId === context.sessionId &&
        active.generation === context.generation &&
        nativeSeedPath(this.app?.graph, node) !== null
      );
    };
    const fieldId = (): string | null => {
      for (const [id, owner] of this.inventory.owners) {
        if (this.relations.snapshot().fields.find((field) => field.id === id)?.semantic !== 'seed')
          continue;
        if (owner.node === node && owner.widget.name === name) return id;
        const candidate = resolveNativeSeedOwner(owner.node, owner.widget.name);
        if (candidate?.node === native.node && candidate.widget === native.widget) return id;
      }
      return null;
    };
    const readLast = (): number | null => {
      if (!current()) return null;
      const id = fieldId();
      return this.history.read(context, id ? this.#relatedInputs(id) : this.#inputs(node, name));
    };
    return {
      readLast,
      reuse: () =>
        this.#run(() => {
          const value = readLast();
          if (value === null) return;
          const id = fieldId();
          if (id) this.relations.reuseSeed(id, value);
          else restoreNativeSeed(this.app?.graph, this.document, node, name, value);
        }),
    };
  }

  /** Reuse a toolbar group's latest accepted participating seed through the same relation edit. */
  group(key: string): SeedReuseControl {
    const readLast = (): number | null => {
      const context = this.sessions.read().active;
      const group = this.relations.snapshot().groups.find((item) => item.key === key);
      return context && group
        ? this.history.read(
            context,
            group.members.flatMap((field) => this.#fieldInputs(field.id)),
          )
        : null;
    };
    return {
      readLast,
      reuse: () =>
        this.#run(() => {
          const value = readLast();
          const group = this.relations.snapshot().groups.find((item) => item.key === key);
          const field = group?.members[0];
          if (value !== null && field && group?.enabled) this.relations.reuseSeed(field.id, value);
        }),
    };
  }

  /** Read all participants of the effective field owner, including a linked master bypassed by the run. */
  #relatedInputs(id: string): readonly SeedInput[] {
    const snapshot = this.relations.snapshot();
    const resolved = snapshot.resolved.find((item) => item.field.id === id);
    const fields = resolved?.override
      ? (snapshot.groups.find((group) => group.key === resolved.override)?.members ?? [])
      : linkedFieldMembers(resolved?.source ?? id, snapshot.fields, snapshot.nodes, snapshot.state);
    return fields.flatMap((field) => this.#fieldInputs(field.id));
  }

  /** Resolve one exposed relation field to its original scalar seed input. */
  #fieldInputs(id: string): readonly SeedInput[] {
    const owner = this.inventory.owners.get(id);
    return owner ? this.#inputs(owner.node, owner.widget.name) : [];
  }

  /** Qualify a promoted seed using its actual leaf path rather than the facade label. */
  #inputs(node: UnknownRecord, name: string): readonly SeedInput[] {
    const owner = resolveNativeSeedOwner(node, name);
    if (!owner || typeof owner.widget.name !== 'string') return [];
    const paths = new Set([
      nativeSeedPath(this.app?.graph, owner.node),
      nativeSeedPath(this.app?.graph, owner.node, true),
    ]);
    return [...paths].flatMap((path) => (path ? [{ path, name: String(owner.widget.name) }] : []));
  }

  /** Keep native menu failures on the existing SugarCubes feedback path. */
  #run(operation: () => void): void {
    try {
      operation();
    } catch (error: unknown) {
      this.reportError(error);
    }
  }
}
