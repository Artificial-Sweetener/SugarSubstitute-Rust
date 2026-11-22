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
/** Capture an admitted portable checkpoint from the current Comfy document and result owners. */
import { desktopCheckpointError, utf8Bytes } from '../../core/embedded/SessionRuntime.js';
import type {
  SessionAsset,
  SessionBinding,
  SessionInput,
  SessionOutput,
  SessionScope,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';
import type {
  SessionCheckpoint,
  SessionEntry,
} from '../../../../../integrations/contracts/SessionCheckpointV2Protocol.generated.js';

import type { ComfyWorkflowExecutionRuntime } from '../../execution/infrastructure/ComfyWorkflowExecutionRuntime.js';
import type { WorkflowOutputIdentities } from '../../execution/application/WorkflowOutputIdentities.js';
import type { WorkflowSubmissionSnapshots } from '../../execution/application/WorkflowSubmissionSnapshots.js';
import type { WorkflowDesktopOutputs } from '../../execution/application/WorkflowDesktopOutputs.js';
import type { CanvasSessionSnapshots } from '../../output_canvas/application/CanvasSessionSnapshots.js';
import type { SettingsWorkspaceRoute } from '../../settings/SettingsWorkspaceRoute.js';
import { readComfyWorkflowInputState } from '../../input_canvas/infrastructure/ComfyWorkflowInputState.js';
import type { DesktopWorkspaceBindings } from '../application/DesktopWorkspaceBindings.js';
import {
  DesktopSessionRefusal,
  type DesktopSessionCancellation,
} from '../domain/DesktopSession.js';
import type { ComfyDesktopDocuments } from './ComfyDesktopDocuments.js';
import type { WorkflowSessionContext } from '../../execution/domain/WorkflowSession.js';
import type { ComfyWorkflowViewPreferences } from './ComfyWorkflowViewPreferences.js';

export interface ComfyDesktopCaptureOptions {
  readonly execution: ComfyWorkflowExecutionRuntime;
  readonly documents: ComfyDesktopDocuments;
  readonly bindings: DesktopWorkspaceBindings;
  readonly outputs: WorkflowDesktopOutputs;
  readonly identities: WorkflowOutputIdentities;
  readonly submissions: WorkflowSubmissionSnapshots;
  readonly canvas: CanvasSessionSnapshots;
  readonly presentation: ComfyWorkflowViewPreferences;
  readonly input: ReadonlyMap<string, SessionInput | null>;
  readonly assets: ReadonlyMap<string, SessionAsset>;
  readonly scope: SessionScope;
  readonly settingsRoute?: SettingsWorkspaceRoute;
  readonly versions: () => Promise<{ readonly frontend: string; readonly sugarcubes: string }>;
  readonly now: () => number;
  readonly previousId: () => string | null;
  readonly assertCurrent: (cancellation: DesktopSessionCancellation) => void;
}

/** Keep checkpoint inventory and bounded DTO admission together; this owner never changes workflow state. */
export class ComfyDesktopCapture {
  /** Each workflow's entry as last captured, kept for a capture that cannot read it again. */
  readonly #kept = new Map<string, SessionEntry>();
  constructor(private readonly options: ComfyDesktopCaptureOptions) {}
  /** Freeze all semantic owners before awaiting receipts; the client refuses any later observation fence. */
  async capture(
    id: string,
    revision: number,
    cancellation: DesktopSessionCancellation,
  ): Promise<SessionCheckpoint> {
    const snapshot = this.options.execution.sessions.read();
    const versions = await this.options.versions();
    const entries: SessionEntry[] = [];
    let total = 0;
    for (const context of snapshot.open) {
      this.options.assertCurrent(cancellation);
      const binding = this.options.bindings.binding(context.sessionId);
      try {
        const entry = await this.#entry(context, binding, total, cancellation);
        total += entry.bytes;
        entries.push(entry.entry);
        this.#kept.set(binding.entryId, entry.entry);
      } catch (error: unknown) {
        // A workflow that changed while it was captured is captured again, not kept.
        if (
          cancellation.cancelled() ||
          (error instanceof DesktopSessionRefusal &&
            (error.code === 'too-large' || error.code === 'stale'))
        )
          throw error;
        // One workflow that cannot be captured keeps the state it was last saved with, so it
        // never costs the other workflows their save.
        const kept = this.#kept.get(binding.entryId);
        if (kept) entries.push(kept);
        console.warn('SugarCubes kept a workflow at its last saved state.', {
          entry: binding.entryId,
          kept: Boolean(kept),
          error,
        });
      }
    }
    const assets = new Map<string, SessionAsset>();
    for (const output of entries.flatMap((entry) => entry.outputs)) {
      if (output.assetDigest === null) continue;
      const known = this.options.assets.get(output.assetDigest);
      const asset = output.file.storage === 'temp' ? requestedTemporaryAsset(output) : known;
      if (!asset)
        throw new DesktopSessionRefusal(
          'identity-mismatch',
          'Retained final asset descriptor is unavailable.',
        );
      const previous = assets.get(asset.digest) ?? known;
      if (
        asset.digest !== output.digest ||
        asset.bytes !== output.bytes ||
        asset.media !== output.media ||
        (previous &&
          (previous.bytes !== asset.bytes ||
            previous.extension !== asset.extension ||
            previous.media !== asset.media))
      )
        throw new DesktopSessionRefusal(
          'identity-mismatch',
          'Final asset receipts disagree about their encoded identity.',
        );
      assets.set(asset.digest, asset);
    }
    const checkpoint: SessionCheckpoint = Object.freeze({
      version: 2,
      id,
      revision,
      previousId: this.options.previousId(),
      capturedAt: this.options.now(),
      scope: this.options.scope,
      producer: 'SugarCubes desktop frontend',
      frontend: versions.frontend,
      sugarcubes: versions.sugarcubes,
      entries: Object.freeze(entries),
      activeEntry: snapshot.active
        ? this.options.bindings.binding(snapshot.active.sessionId).entryId
        : null,
      route: this.options.settingsRoute?.read() ?? 'workflow',
      placement: null,
      assets: Object.freeze([...assets.values()]),
    });
    const rule = desktopCheckpointError(checkpoint);
    if (rule !== null)
      throw new DesktopSessionRefusal(
        'invalid',
        `Desktop checkpoint failed semantic admission: ${rule}.`,
      );
    return checkpoint;
  }
  /** Capture one open workflow's entry, refusing when it would overflow the checkpoint. */
  async #entry(
    context: WorkflowSessionContext,
    binding: SessionBinding,
    total: number,
    cancellation: DesktopSessionCancellation,
  ): Promise<{ readonly bytes: number; readonly entry: SessionEntry }> {
    const document = this.options.documents.capture(context, binding.graphRevision);
    const bytes = utf8Bytes(document.graphJson) + utf8Bytes(document.baselineGraphJson ?? '');
    if (total + bytes > 128 * 1024 * 1024)
      throw new DesktopSessionRefusal(
        'too-large',
        'Desktop workspace exceeds checkpoint admission.',
      );
    const result = this.options.execution.results.read(context.sessionId);
    const preferences = this.options.presentation.capture(context.sessionId);
    if (!result || !preferences)
      throw new DesktopSessionRefusal('stale', 'Desktop workflow owner was retired.');
    const outputs = await this.options.outputs.capture(
      binding,
      this.options.scope.backend,
      result,
      cancellation,
    );
    this.options.assertCurrent(cancellation);
    // Results arriving while receipts were awaited can advance this workflow past the captured
    // document; the capture then describes no single moment and is retried, not refused.
    if (!sameBinding(this.options.bindings.binding(context.sessionId), binding))
      throw new DesktopSessionRefusal(
        'stale',
        'Desktop workflow changed while its checkpoint was captured.',
      );
    const route =
      readComfyWorkflowInputState(JSON.parse(document.graphJson) as unknown)?.route ?? 'output';
    const view = Object.freeze({
      ...this.options.canvas.capture(
        context,
        binding,
        outputs,
        this.options.identities,
        preferences,
        route,
      ),
      scroll: this.options.presentation.captureScroll(context.sessionId),
    });
    return {
      bytes,
      entry: Object.freeze({
        id: binding.entryId,
        document,
        input: this.options.input.get(binding.entryId) ?? null,
        view,
        sourceRevision: binding.sourceRevision,
        navigationRevision: binding.navigationRevision,
        sources: this.options.identities.inventory(binding.entryId),
        outputs,
        submissions: this.options.submissions.read(binding.entryId),
      }),
    };
  }
}

/** Describe exact temporary bytes the native publisher must verify and retain before durable acknowledgement. */
function requestedTemporaryAsset(output: SessionOutput): SessionAsset {
  const extensions: Readonly<Record<string, SessionAsset['extension']>> = {
    'image/png': 'png',
    'image/jpeg': 'jpg',
    'image/webp': 'webp',
    'image/gif': 'gif',
    'video/mp4': 'mp4',
    'video/webm': 'webm',
  };
  const extension = Object.hasOwn(extensions, output.mime) ? extensions[output.mime] : undefined;
  if (!extension)
    throw new DesktopSessionRefusal('unavailable', 'Temporary final format cannot be retained.');
  return Object.freeze({
    version: 1,
    digest: output.digest,
    bytes: output.bytes,
    extension,
    media: output.media,
  });
}

/** Whether a workflow still sits at the exact revisions a capture started from. */
function sameBinding(left: SessionBinding, right: SessionBinding): boolean {
  return (
    left.entryId === right.entryId &&
    left.runtimeSession === right.runtimeSession &&
    left.generation === right.generation &&
    left.graphRevision === right.graphRevision &&
    left.sourceRevision === right.sourceRevision &&
    left.navigationRevision === right.navigationRevision
  );
}
