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
/** Own one cancelled-or-committed Comfy workspace materialization transaction. */
import type {
  SessionAsset,
  SessionBinding,
  SessionCheckpoint,
  SessionInput,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';
import type { ComfyWorkflowExecutionRuntime } from '../../execution/infrastructure/ComfyWorkflowExecutionRuntime.js';
import type { WorkflowOutputIdentities } from '../../execution/application/WorkflowOutputIdentities.js';
import type { WorkflowSubmissionSnapshots } from '../../execution/application/WorkflowSubmissionSnapshots.js';
import { restoreDesktopResults } from '../../execution/application/WorkflowDesktopResultImport.js';
import type { DesktopOutputArtifacts } from '../../execution/domain/DesktopOutputArtifacts.js';
import { decodeComfyWorkflowSources } from '../../execution/infrastructure/ComfyWorkflowSourceDecoder.js';
import type { CanvasSessionSnapshots } from '../../output_canvas/application/CanvasSessionSnapshots.js';
import type { CanvasRoutePort } from '../../output_canvas/domain/CanvasRoute.js';
import type { SettingsWorkspaceRoute } from '../../settings/SettingsWorkspaceRoute.js';
import type { DesktopWorkspaceBindings } from '../application/DesktopWorkspaceBindings.js';
import {
  DesktopSessionRefusal,
  type DesktopSessionCancellation,
} from '../domain/DesktopSession.js';
import type { ComfyDesktopDocuments } from './ComfyDesktopDocuments.js';
import type { ComfyWorkflowViewPreferences } from './ComfyWorkflowViewPreferences.js';

export interface ComfyDesktopMaterializationOptions {
  readonly execution: ComfyWorkflowExecutionRuntime;
  readonly documents: ComfyDesktopDocuments;
  readonly bindings: DesktopWorkspaceBindings;
  readonly identities: WorkflowOutputIdentities;
  readonly submissions: WorkflowSubmissionSnapshots;
  readonly artifacts: DesktopOutputArtifacts;
  readonly input: Map<string, SessionInput | null>;
  readonly assets: Map<string, SessionAsset>;
  readonly canvas: CanvasSessionSnapshots;
  readonly presentation: ComfyWorkflowViewPreferences;
  readonly route?: CanvasRoutePort;
  readonly settingsRoute?: SettingsWorkspaceRoute;
  readonly publishObservation: () => Promise<void>;
  readonly refresh: (prime: boolean) => void;
  readonly assertCurrent: (cancellation: DesktopSessionCancellation) => void;
  readonly retired: () => boolean;
  readonly superseded: () => void;
}

/** Keep fresh binding proposal, metadata reads, restore-only publication and acknowledgement ordering together. */
export class ComfyDesktopMaterialization {
  #phase: 'documents' | 'outputs' | null = null;
  #cancellation: DesktopSessionCancellation | null = null;
  #restoringSettings = false;
  constructor(private readonly options: ComfyDesktopMaterializationOptions) {}
  active(): boolean {
    return this.#phase !== null;
  }
  resolving(): boolean {
    return this.#phase === 'outputs';
  }
  restoringSettings(): boolean {
    return this.#restoringSettings;
  }
  cancel(): void {
    this.#cancellation?.cancel();
  }
  /**
   * Bind host-adopted durable entries to the tabs Comfy itself restored, then import references
   * and view intent. Comfy owns the tabs: an entry whose tab Comfy did not reopen is left out, and
   * the returned bindings name only the entries that were bound.
   */
  async materialize(
    checkpoint: SessionCheckpoint,
    cancellation: DesktopSessionCancellation,
  ): Promise<readonly SessionBinding[]> {
    this.#phase = 'documents';
    this.#cancellation = cancellation;
    this.options.bindings.changed();
    try {
      const contexts = this.options.documents.match(
        checkpoint.entries,
        () => !cancellation.cancelled() && !this.options.retired(),
      );
      const entries = checkpoint.entries.filter((entry) => contexts.has(entry.id));
      this.options.assertCurrent(cancellation);
      this.options.bindings.synchronize(this.options.execution.sessions.read());
      for (const entry of entries) {
        const context = contexts.get(entry.id);
        if (!context)
          throw new DesktopSessionRefusal('stale', 'Restored Comfy workflow binding was retired.');
        this.options.bindings.bind({
          entryId: entry.id,
          runtimeSession: context.sessionId,
          generation: context.generation,
          graphRevision: entry.document.graphRevision,
          sourceRevision: entry.sourceRevision,
          navigationRevision: entry.navigationRevision,
        });
        this.options.identities.restoreSources(entry.id, entry.sources);
        this.options.identities.restoreResults(entry.id, entry.submissions, entry.outputs);
        this.options.submissions.restore(entry.id, entry.submissions);
        this.options.input.set(entry.id, entry.input);
      }
      this.options.assets.clear();
      for (const asset of checkpoint.assets) this.options.assets.set(asset.digest, asset);
      this.#phase = 'outputs';
      await this.options.publishObservation();
      this.options.assertCurrent(cancellation);
      for (const entry of entries) {
        this.options.assertCurrent(cancellation);
        const context = contexts.get(entry.id);
        if (!context)
          throw new DesktopSessionRefusal('stale', 'Restored occurrence is unavailable.');
        const binding = this.options.bindings.binding(context.sessionId);
        const restored = await restoreDesktopResults(
          this.options.artifacts,
          entry,
          binding,
          context,
          checkpoint.id,
          cancellation,
        );
        this.options.assertCurrent(cancellation);
        if (!this.options.execution.results.restoreSession(context, restored))
          throw new DesktopSessionRefusal(
            'stale',
            'A genuine live run superseded desktop materialization.',
          );
        this.options.execution.restoreSubmissionOrder(context.sessionId, restored.runOrder);
        this.options.presentation.restore(context.sessionId, {
          canvas: entry.view.canvasEnabled,
          masonry: entry.view.masonryEnabled,
          editorWidth: entry.view.editorWidth,
        });
        this.options.canvas.restore(
          context,
          entry,
          decodeComfyWorkflowSources(JSON.parse(entry.document.graphJson) as unknown),
        );
      }
      const shown = this.options.execution.sessions.read().active?.sessionId;
      const active = entries.find((entry) => contexts.get(entry.id)?.sessionId === shown);
      if (active) this.options.route?.choose(active.view.route);
      if (checkpoint.route === 'settings') {
        if (!this.options.settingsRoute)
          throw new DesktopSessionRefusal(
            'unavailable',
            'Settings workspace route is unavailable.',
          );
        this.#restoringSettings = true;
        const controller = new AbortController();
        const stop = cancellation.subscribe(() => controller.abort());
        try {
          await this.options.settingsRoute.show(controller.signal);
        } finally {
          stop();
          this.#restoringSettings = false;
        }
        this.options.assertCurrent(cancellation);
      }
      this.#phase = null;
      this.options.refresh(true);
      const restored = new Set(entries.map((entry) => entry.id));
      return this.options.bindings
        .fence()
        .bindings.filter((binding) => restored.has(binding.entryId));
    } catch (error: unknown) {
      if (error instanceof DesktopSessionRefusal && error.code === 'stale')
        this.options.superseded();
      throw error;
    } finally {
      this.#phase = null;
      this.#cancellation = null;
      this.options.refresh(false);
      this.options.bindings.changed();
    }
  }
}
