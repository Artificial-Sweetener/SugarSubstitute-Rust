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
/** One finite native owner publishes authored names and admits current host synchronization through supported transactions. */
import { Facet, Transaction } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { changedRegionIdentities, type RegionName } from '../../core/src/index.js';
import { captureRegionNames } from '../../services/src/regions/names.js';
import { editorFreshness, freshnessRejection, type EditorFreshness } from './freshness.js';
import { documentKind } from './document-kind.js';
import {
  prepareNativeStructure,
  semanticStructures,
  installSemanticStructure,
} from './structure.js';
import { sharedRegionNameEdit, synchronizedRegionNames } from './region-host-transaction.js';
import {
  regionNameTargetBrand,
  type RegionNameTarget,
  type RegionNameInspection,
  type RegionNameSyncOutcome,
  type RegionNameChange,
} from './region-host-contracts.js';

/** Mounted revision authority and optional host observer have the same finite native view lifetime. */
export interface RegionNameObserver {
  readonly revision: () => number;
  readonly observe: ((change: RegionNameChange) => void) | undefined;
}
/** Consumers use mountEditor; one facade installs the immutable observer/revision adapter. */
export const regionNameObserver = Facet.define<RegionNameObserver, RegionNameObserver | null>({
  /** Duplicate publication owners are observable rather than silently selected. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple region name observers');
    return values[0] ?? null;
  },
});

/** Previous published names are derived transition metadata, not a second prompt, caret or history. */
class RegionHost {
  private readonly view: EditorView;
  private previous: readonly RegionName[] | null = null;
  private targets = new WeakMap<RegionNameTarget, EditorFreshness>();
  private consumed = new WeakSet<RegionNameTarget>();
  private generation = 0;
  private disposed = false;
  /** Observe the already mounted view; initial semantic publication seeds a baseline. */
  constructor(view: EditorView) {
    this.view = view;
    if (view.state.facet(documentKind) === 'prompt') {
      const prepared = view.state.field(semanticStructures).native;
      if (prepared?.kind !== 'prompt') throw new Error('Regional baseline owner unavailable');
      this.previous = prepared.regionNames;
    }
  }

  /** Current semantic publications drive authored transitions; peer synchronization seeds names without an echo. */
  update(update: ViewUpdate): void {
    if (update.startState.field(editorFreshness) !== update.state.field(editorFreshness))
      this.generation++;
    for (const transaction of update.transactions) {
      const names = transaction.annotation(synchronizedRegionNames);
      if (names) {
        this.generation++;
        this.previous = names;
      }
    }
    if (update.state.facet(documentKind) !== 'prompt') {
      this.previous = null;
      return;
    }
    const semantic = update.state.field(semanticStructures).native;
    const current = semantic?.kind === 'prompt' ? semantic.regionNames : null;
    if (!current || current === this.previous) return;
    const observer = update.state.facet(regionNameObserver);
    if (!this.previous || !observer?.observe) {
      this.previous = current;
      return;
    }
    const changes = changedRegionIdentities(this.previous, current);
    if (!changes.length) {
      this.previous = current;
      return;
    }
    const freshness = update.state.field(editorFreshness),
      generation = ++this.generation;
    queueMicrotask(() => {
      if (
        this.disposed ||
        generation !== this.generation ||
        this.view.state.field(editorFreshness) !== freshness
      )
        return;
      this.previous = current;
      observer.observe?.(
        Object.freeze({
          sourceRevision: observer.revision(),
          changes,
          names: current,
        }),
      );
    });
  }

  /** Explicit host inspection captures one same-editor source/context handle outside native input. */
  inspect(): RegionNameInspection {
    if (this.view.composing || this.view.compositionStarted) return { status: 'composing' };
    const kind = this.view.state.facet(documentKind);
    if (kind !== 'prompt') return { status: 'unsupported', documentKind: kind };
    const prepared = prepareNativeStructure(this.view.state);
    if (prepared.kind !== 'prompt') throw new Error('regional-inspection-kind-mismatch');
    if (prepared !== this.view.state.field(semanticStructures).native)
      this.view.dispatch({
        effects: installSemanticStructure.of({ purpose: 'native', prepared }),
        annotations: Transaction.addToHistory.of(false),
      });
    const target: RegionNameTarget = Object.freeze({
      kind: 'region-name-target',
      [regionNameTargetBrand]: true as const,
    });
    this.targets.set(target, this.view.state.field(editorFreshness));
    const observer = this.view.state.facet(regionNameObserver);
    if (!observer) throw new Error('Region revision owner missing');
    return {
      status: 'ready',
      sourceRevision: observer.revision(),
      names: prepared.regionNames,
      target,
    };
  }

  /** Current host loading maps native selection/history through one minimal marker-name transaction, including read-only peers. */
  synchronize(names: readonly RegionName[], target: RegionNameTarget): RegionNameSyncOutcome {
    if (this.disposed) return { status: 'rejected', reason: 'disposed' };
    const captured = this.targets.get(target);
    if (!captured)
      return {
        status: 'rejected',
        reason: this.consumed.has(target) ? 'consumed-command' : 'foreign-command',
      };
    if (this.view.composing || this.view.compositionStarted)
      return { status: 'rejected', reason: 'composing' };
    const accepted = captureRegionNames(names);
    this.targets.delete(target);
    this.consumed.add(target);
    const failure = freshnessRejection(this.view.state.field(editorFreshness), captured);
    if (failure) return { status: 'rejected', reason: failure };
    const prepared = prepareNativeStructure(this.view.state);
    const transaction = this.view.state.update(
      sharedRegionNameEdit(this.view.state, prepared, accepted),
    );
    this.view.dispatch(transaction);
    return { status: transaction.docChanged ? 'applied' : 'unchanged' };
  }

  /** Teardown retires pending notifications and caller-held handles without retaining source payloads. */
  destroy(): void {
    this.disposed = true;
    this.generation++;
    this.previous = null;
    this.targets = new WeakMap();
    this.consumed = new WeakSet();
  }
}
/** One supported native lifetime owns publication and current synchronization; Vue observes through public methods/events. */
export const regionHost = ViewPlugin.fromClass(RegionHost);
