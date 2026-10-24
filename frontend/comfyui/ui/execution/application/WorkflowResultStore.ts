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
/** Own session-qualified latest media, revisions and execution status without browser allocations. */

import {
  outputResultId,
  snapshotOutputResultId,
  type WorkflowSessionId,
  type OutputSourceId,
} from '../domain/ExecutionIdentity.js';
import type { WorkflowSessionContext } from '../domain/WorkflowSession.js';
import type { WorkflowRun } from '../domain/WorkflowRun.js';
import type { WorkflowMediaPayload } from '../domain/WorkflowMedia.js';
import { combineWorkflowMedia, freezeWorkflowMedia } from '../domain/WorkflowMedia.js';
import type { WorkflowResultSession } from '../domain/WorkflowResultSession.js';
import type { WorkflowPreview } from '../domain/WorkflowPreview.js';
import type {
  OutputProvenance,
  WorkflowOutputSet,
  WorkflowOutputSource,
  WorkflowOutputItem,
} from '../domain/WorkflowOutput.js';

export type WorkflowResultChange = 'state' | 'output' | 'restore';

interface RetainedOutput {
  readonly output: WorkflowOutputSet;
  readonly payload: WorkflowMediaPayload;
  readonly delivery: string;
  /** Keep first-member preview retirement monotonic through metadata-only replacement. */
  readonly completedPreviewRunOrder: number;
}
interface ResultSession {
  snapshot: WorkflowResultSession;
  readonly outputs: Map<OutputSourceId, RetainedOutput>;
}
export interface WorkflowResultUpdate {
  readonly run: WorkflowRun;
  readonly source: WorkflowOutputSource;
  readonly payload: WorkflowMediaPayload;
  readonly containsImages: boolean;
  readonly merge: boolean;
  readonly delivery: string;
  readonly expectedRevision?: number;
}

/** Retain bounded source descriptors and a monotonic visual handoff for each workflow. */
export class WorkflowResultStore {
  readonly #sessions = new Map<WorkflowSessionId, ResultSession>();
  readonly #listeners = new Set<
    (sessionId: WorkflowSessionId, reason: WorkflowResultChange) => void
  >();

  /** Synchronize open bindings and invalidate results when the graph generation changes. */
  synchronize(contexts: readonly WorkflowSessionContext[]): readonly WorkflowSessionId[] {
    const open = new Set(contexts.map((context) => context.sessionId));
    const closed: WorkflowSessionId[] = [];
    for (const id of this.#sessions.keys())
      if (!open.has(id)) {
        this.#sessions.delete(id);
        closed.push(id);
        this.#notify(id);
      }
    for (const context of contexts) {
      const previous = this.#sessions.get(context.sessionId);
      if (previous?.snapshot.context.generation === context.generation) {
        if (
          previous.snapshot.context.title !== context.title ||
          previous.snapshot.context.path !== context.path
        )
          this.#publish(previous, { ...previous.snapshot, context });
        continue;
      }
      const session: ResultSession = {
        outputs: new Map(),
        snapshot: Object.freeze({
          context: Object.freeze({ ...context }),
          revision: (previous?.snapshot.revision ?? 0) + 1,
          outputs: Object.freeze([]),
          runOrder: 0,
          presentedRunOrder: 0,
          runStatus: 'idle',
          preview: null,
          recovery: Object.freeze({ state: 'idle', message: '' }),
        }),
      };
      this.#sessions.set(context.sessionId, session);
      this.#notify(context.sessionId);
    }
    return Object.freeze(closed);
  }

  /** Read immutable final references and transient metadata for one explicit workflow. */
  read(sessionId: WorkflowSessionId): WorkflowResultSession | null {
    return this.#sessions.get(sessionId)?.snapshot ?? null;
  }

  /** Enumerate current lightweight states for newly attached view-capable canvases. */
  snapshots(): readonly WorkflowResultSession[] {
    return [...this.#sessions.values()].map((session) => session.snapshot);
  }

  /** Read one source's revision for exact-history compare-and-set reconciliation. */
  sourceRevision(sessionId: WorkflowSessionId, sourceId: OutputSourceId): number {
    return this.#sessions.get(sessionId)?.outputs.get(sourceId)?.output.provenance.revision ?? 0;
  }

  /** Publish a known run without allowing an older delayed job to displace the execution frontier. */
  updateRun(run: WorkflowRun): void {
    const session = this.#session(run.session);
    if (!session || run.order < session.snapshot.runOrder) return;
    const previous = session.snapshot;
    if (
      run.order === previous.runOrder &&
      ['complete', 'cancelled', 'error'].includes(previous.runStatus)
    )
      return;
    const terminal =
      run.status === 'complete' || run.status === 'cancelled' || run.status === 'error';
    const preview = terminal || run.order > previous.runOrder ? null : previous.preview;
    if (
      previous.runOrder === run.order &&
      previous.runStatus === run.status &&
      previous.preview === preview
    )
      return;
    this.#publish(session, {
      ...previous,
      runOrder: run.order,
      runStatus: run.status,
      preview,
      recovery:
        run.order > previous.runOrder
          ? Object.freeze({ state: 'idle', message: '' })
          : previous.recovery,
    });
  }

  /** Expose exact-history availability without replacing a valid selected final or another run's status. */
  updateRecovery(
    run: WorkflowRun,
    state: WorkflowResultSession['recovery']['state'],
    message = '',
  ): void {
    const session = this.#session(run.session);
    if (!session || run.order < session.snapshot.runOrder) return;
    this.#publish(session, { ...session.snapshot, recovery: Object.freeze({ state, message }) });
  }

  /** Apply validated replace/merge semantics only for a still-open generation and a current source revision. */
  retain(update: WorkflowResultUpdate): boolean {
    const session = this.#session(update.run.session);
    if (!session) return false;
    const previous = session.outputs.get(update.source.id);
    const oldRevision = previous?.output.provenance.revision ?? 0;
    if (
      update.expectedRevision !== undefined &&
      update.expectedRevision !== oldRevision &&
      previous &&
      previous.output.provenance.runOrder >= update.run.order
    )
      return false;
    if (previous && update.run.order < previous.output.provenance.runOrder) return false;
    const sameRun = previous?.output.provenance.promptId === update.run.promptId;
    const delivery = JSON.stringify([update.delivery, update.merge]);
    if (sameRun && previous.delivery === delivery) return false;
    const inputPreview =
      previous &&
      previous.payload.images.length > 0 &&
      previous.payload.images.every((entry) => entry?.file.type === 'input');
    const payload = combineWorkflowMedia(
      sameRun || inputPreview ? previous.payload : null,
      update.payload,
      update.merge,
      update.containsImages,
    );
    const provenance: OutputProvenance = Object.freeze({
      kind: 'execution',
      sessionId: update.run.session.sessionId,
      promptId: update.run.promptId,
      sourceId: update.source.id,
      executionPath: update.source.executionPath,
      generation: update.run.session.generation,
      runOrder: update.run.order,
      sourceOrder: update.source.order,
      revision: oldRevision + 1,
    });
    const carried =
      !update.containsImages &&
      !update.payload.videos.length &&
      !update.payload.models.length &&
      inputPreview
        ? previous.output.items.filter((item) => item.batchIndex < previous.payload.images.length)
        : [];
    this.#retain(session, update.source, provenance, payload, delivery, carried);
    return true;
  }

  /** Restore only an explicitly named host workflow snapshot and never fabricate a prompt ID. */
  restore(
    context: WorkflowSessionContext,
    source: WorkflowOutputSource,
    payload: WorkflowMediaPayload,
    snapshotId: string,
  ): boolean {
    const session = this.#session(context);
    if (!session || session.outputs.has(source.id)) return false;
    const provenance: OutputProvenance = Object.freeze({
      kind: 'workflow-snapshot',
      promptId: null,
      snapshotId,
      sessionId: context.sessionId,
      sourceId: source.id,
      executionPath: source.executionPath,
      generation: context.generation,
      runOrder: 0,
      sourceOrder: source.order,
      revision: 1,
    });
    this.#retain(
      session,
      source,
      provenance,
      freezeWorkflowMedia(payload),
      'host-workflow-snapshot',
    );
    return true;
  }

  /** Publish only the current known run's latest source preview; bytes are never retained here. */
  publishPreview(preview: WorkflowPreview): boolean {
    const session = this.#sessions.get(preview.sessionId);
    if (
      !session ||
      (session.outputs.get(preview.sourceId)?.completedPreviewRunOrder ?? -1) >= preview.runOrder ||
      session.snapshot.context.generation !== preview.generation ||
      preview.runOrder < session.snapshot.runOrder ||
      (preview.runOrder === session.snapshot.runOrder &&
        ['complete', 'cancelled', 'error'].includes(session.snapshot.runStatus)) ||
      (session.snapshot.preview?.promptId === preview.promptId &&
        session.snapshot.preview.sequence >= preview.sequence)
    )
      return false;
    this.#publish(session, {
      ...session.snapshot,
      preview: Object.freeze({
        ...preview,
        dimensions: preview.dimensions ? Object.freeze({ ...preview.dimensions }) : null,
      }),
      runOrder: preview.runOrder,
      presentedRunOrder: Math.max(session.snapshot.presentedRunOrder, preview.runOrder),
      runStatus: 'running',
    });
    return true;
  }

  /** Release transient preview metadata when its owning surface stops displaying it. */
  clearPreview(sessionId: WorkflowSessionId): void {
    const session = this.#sessions.get(sessionId);
    if (session?.snapshot.preview) this.#publish(session, { ...session.snapshot, preview: null });
  }

  /** Subscribe to affected workflow identities rather than unscoped card or graph refresh requests. */
  subscribe(
    listener: (sessionId: WorkflowSessionId, reason: WorkflowResultChange) => void,
  ): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Release all descriptors and subscriptions with the integration lifetime. */
  dispose(): void {
    this.#sessions.clear();
    this.#listeners.clear();
  }

  /** Match explicit binding generation so old loads and closed tabs cannot revive results. */
  #session(context: WorkflowSessionContext): ResultSession | null {
    const session = this.#sessions.get(context.sessionId);
    return session?.snapshot.context.generation === context.generation ? session : null;
  }

  /** Preserve carried input identities while constructing attributable new media and unavailable positions. */
  #retain(
    session: ResultSession,
    source: WorkflowOutputSource,
    provenance: OutputProvenance,
    payload: WorkflowMediaPayload,
    delivery: string,
    carried: readonly WorkflowOutputItem[] = [],
  ): void {
    const media = [...payload.images, ...payload.videos, ...payload.models];
    const items: WorkflowOutputItem[] = [];
    const preserved = new Map(carried.map((item) => [item.batchIndex, item]));
    media.forEach((entry, batchIndex) => {
      if (!entry) return;
      const existing = preserved.get(batchIndex);
      if (existing) {
        items.push(existing);
        return;
      }
      const id =
        provenance.kind === 'execution'
          ? outputResultId(
              provenance.sessionId,
              provenance.promptId,
              source.id,
              provenance.revision,
              batchIndex,
            )
          : snapshotOutputResultId(
              provenance.sessionId,
              provenance.snapshotId,
              source.id,
              provenance.revision,
              batchIndex,
            );
      items.push(Object.freeze({ id, provenance, batchIndex, ...entry }));
    });
    const output: WorkflowOutputSet = Object.freeze({
      source: Object.freeze({ ...source, breadcrumb: Object.freeze([...source.breadcrumb]) }),
      provenance,
      items: Object.freeze(items),
      text: payload.text,
      batchCount: media.length,
    });
    const completedPreviewRunOrder = items.reduce(
      (latest, item) =>
        item.batchIndex === 0 && item.file.type !== 'input'
          ? Math.max(latest, item.provenance.runOrder)
          : latest,
      session.outputs.get(source.id)?.completedPreviewRunOrder ?? -1,
    );
    session.outputs.set(source.id, { output, payload, delivery, completedPreviewRunOrder });
    const outputs = Object.freeze(
      [...session.outputs.values()]
        .map((retained) => retained.output)
        .sort((a, b) => a.source.order - b.source.order),
    );
    const preview = session.snapshot.preview;
    this.#publish(
      session,
      {
        ...session.snapshot,
        outputs,
        presentedRunOrder: items.reduce(
          (latest, item) =>
            item.file.type === 'input' ? latest : Math.max(latest, item.provenance.runOrder),
          session.snapshot.presentedRunOrder,
        ),
        preview:
          preview?.sourceId === source.id && completedPreviewRunOrder >= preview.runOrder
            ? null
            : preview,
      },
      provenance.kind === 'execution' ? 'output' : 'restore',
    );
  }

  /** Increment one coherent session snapshot revision before notifying its subscribers. */
  #publish(
    session: ResultSession,
    snapshot: WorkflowResultSession,
    reason: WorkflowResultChange = 'state',
  ): void {
    session.snapshot = Object.freeze({
      ...snapshot,
      context: Object.freeze({ ...snapshot.context }),
      revision: session.snapshot.revision + 1,
    });
    this.#notify(snapshot.context.sessionId, reason);
  }

  /** Identify the affected workflow so consumers can update only mounted surfaces. */
  #notify(sessionId: WorkflowSessionId, reason: WorkflowResultChange = 'state'): void {
    for (const listener of this.#listeners) listener(sessionId, reason);
  }
}
