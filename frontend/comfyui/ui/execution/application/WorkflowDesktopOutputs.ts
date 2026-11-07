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
/** Coordinate exact immutable output receipts with the existing result, source and accepted-submission owners. */
import type {
  SessionBinding,
  SessionOutput,
  SessionScene,
  SessionSource,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';
import type { DesktopOutputArtifacts } from '../domain/DesktopOutputArtifacts.js';
import type { WorkflowResultSession } from '../domain/WorkflowResultSession.js';
import type {
  WorkflowOutputItem,
  WorkflowOutputSet,
  WorkflowOutputSource,
} from '../domain/WorkflowOutput.js';
import {
  DesktopSessionRefusal,
  type DesktopSessionCancellation,
} from '../../workflow/domain/DesktopSession.js';
import type { WorkflowOutputIdentities } from './WorkflowOutputIdentities.js';
import type { WorkflowSubmissionSnapshots } from './WorkflowSubmissionSnapshots.js';

/** Materialize only references and facts; byte ownership stays with the artifact adapter. */
export class WorkflowDesktopOutputs {
  readonly #snapshotSources = new WeakMap<WorkflowOutputSet, SessionSource>();
  constructor(
    private readonly identities: WorkflowOutputIdentities,
    private readonly submissions: WorkflowSubmissionSnapshots,
    private readonly artifacts: DesktopOutputArtifacts,
  ) {}

  /** Retain only current graph sources, visible/scene finals and accepted pending/current associations. */
  retain(
    binding: SessionBinding,
    result: WorkflowResultSession,
    outstandingPrompts: ReadonlySet<string>,
  ): void {
    const sets = [
      ...result.outputs,
      ...(result.scenes?.scenes.flatMap((scene) => scene.outputs) ?? []),
    ];
    const items = sets.flatMap((output) => output.items);
    const prompts = new Set(outstandingPrompts);
    for (const item of items)
      if (item.provenance.promptId) prompts.add(item.provenance.promptId.slice('prompt:'.length));
    const accepted = this.submissions.read(binding.entryId);
    const latest = accepted.at(-1);
    if (latest) prompts.add(latest.promptId);
    const group = result.scenes
      ? this.identities.scene(binding.entryId, result.scenes.id, null)
      : latest?.scene?.groupId;
    if (group)
      for (const submission of accepted)
        if (submission.scene?.groupId === group) prompts.add(submission.promptId);
    this.submissions.retain(binding.entryId, prompts);
    const retained = this.submissions.read(binding.entryId);
    const sources = new Set(
      this.identities
        .inventory(binding.entryId)
        .filter((source) => source.graphRevision === binding.graphRevision)
        .map((source) => source.id),
    );
    for (const set of sets)
      if (set.provenance.kind === 'workflow-snapshot') {
        let source = this.#snapshotSources.get(set);
        if (!source) {
          source = this.identities.sources(binding, [set.source])[0];
          if (source) this.#snapshotSources.set(set, source);
        }
        if (source) sources.add(source.id);
      }
    const runs = new Set(retained.map((submission) => submission.runId));
    const outputs = new Set<string>();
    const scenes = new Set<string>();
    for (const submission of retained) {
      for (const source of submission.sources) sources.add(source.id);
      if (submission.scene) {
        scenes.add(submission.scene.groupId);
        scenes.add(submission.scene.sceneId);
      }
    }
    for (const item of items) {
      outputs.add(this.identities.output(binding.entryId, item.id));
      const artifact = item.artifact?.output;
      if (artifact) {
        sources.add(artifact.sourceId);
        runs.add(artifact.runId);
        if (artifact.scene) {
          scenes.add(artifact.scene.groupId);
          scenes.add(artifact.scene.sceneId);
        }
      } else if (item.provenance.kind === 'workflow-snapshot')
        runs.add(this.identities.run(binding.entryId, `snapshot:${item.provenance.snapshotId}`));
    }
    if (result.scenes) {
      scenes.add(this.identities.scene(binding.entryId, result.scenes.id, null));
      for (const scene of result.scenes.scenes)
        scenes.add(this.identities.scene(binding.entryId, result.scenes.id, scene.key));
    }
    this.identities.retainMembership(binding.entryId, sources, runs, outputs, scenes);
  }

  /** Capture every retained final once, including scene finals outside the currently selected scene. */
  async capture(
    binding: SessionBinding,
    backend: string,
    result: WorkflowResultSession,
    cancellation: DesktopSessionCancellation,
  ): Promise<readonly SessionOutput[]> {
    const retained = new Map<
      string,
      {
        item: WorkflowOutputItem;
        source: WorkflowOutputSource;
        set: WorkflowOutputSet;
        scene: SessionScene | null;
        listIndex: number;
      }
    >();
    const collect = (sets: readonly WorkflowOutputSet[], scene: SessionScene | null): void => {
      for (const set of sets)
        for (const [listIndex, item] of set.items.entries()) {
          const previous = retained.get(item.id);
          retained.set(item.id, {
            item,
            source: set.source,
            set,
            scene: scene ?? previous?.scene ?? item.artifact?.output.scene ?? null,
            listIndex,
          });
        }
    };
    collect(result.outputs, null);
    if (result.scenes)
      for (const scene of result.scenes.scenes)
        collect(scene.outputs, {
          groupId: this.identities.scene(binding.entryId, result.scenes.id, null),
          sceneId: this.identities.scene(binding.entryId, result.scenes.id, scene.key),
          order: scene.order,
          count: result.scenes.scenes.length,
          title: scene.title,
        });
    const outputs: SessionOutput[] = [];
    for (const { item, source, set, scene, listIndex } of retained.values()) {
      assertCurrent(cancellation);
      if (item.artifact) {
        if (item.artifact.output.entryId !== binding.entryId)
          throw new DesktopSessionRefusal(
            'identity-mismatch',
            'Retained final belongs to another workflow occurrence.',
          );
        outputs.push(item.artifact.output);
        continue;
      }
      const provenance = item.provenance;
      const prompt = provenance.promptId?.slice('prompt:'.length) ?? null;
      const capturedSource = prompt
        ? this.submissions.source(binding.entryId, prompt, provenance.executionPath)
        : (this.#snapshotSources.get(set) ?? this.identities.sources(binding, [source])[0]);
      if (!capturedSource)
        throw new DesktopSessionRefusal(
          'identity-mismatch',
          'Final output has no accepted immutable source association.',
        );
      const resolved = await this.artifacts.receipt(item.file, cancellation);
      assertCurrent(cancellation);
      if (!resolved.available)
        throw new DesktopSessionRefusal(
          'unavailable',
          `Final output receipt is unavailable: ${resolved.code}.`,
        );
      const receipt = resolved.receipt;
      outputs.push(
        Object.freeze({
          id: this.identities.output(binding.entryId, item.id),
          entryId: binding.entryId,
          backend,
          promptId: prompt,
          runId: this.identities.run(
            binding.entryId,
            prompt ??
              `snapshot:${provenance.kind === 'workflow-snapshot' ? provenance.snapshotId : item.id}`,
          ),
          runOrder: provenance.runOrder,
          sourceId: capturedSource.id,
          scene,
          listIndex,
          batchIndex: item.batchIndex,
          imageIndex: listIndex,
          file: Object.freeze({ ...receipt.file }),
          assetDigest: receipt.file.storage === 'temp' ? receipt.digest : null,
          digest: receipt.digest,
          bytes: receipt.bytes,
          width: receipt.width,
          height: receipt.height,
          media: receipt.media,
          mime: receipt.mime,
          metadataEmbedded: receipt.metadataEmbedded,
        }),
      );
    }
    return Object.freeze(outputs);
  }
}

/** Retire external work before any new checkpoint or runtime mutation. */
function assertCurrent(cancellation: DesktopSessionCancellation): void {
  if (cancellation.cancelled())
    throw new DesktopSessionRefusal('cancelled', 'Desktop artifact operation was cancelled.');
}
