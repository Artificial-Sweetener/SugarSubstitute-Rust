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
/** Admit coherent checkpoint membership, final outputs and image-relative presentation. */
import type {
  SessionCheckpoint,
  SessionEntry,
  SessionOutput,
  SessionSelection,
} from '../contracts/SessionProtocol.generated.js';
import { isSessionCheckpoint } from './SessionValidation.generated.js';
import { sessionDocumentError } from './SessionDocuments.js';
import { sessionPlacementError } from './SessionPlacement.js';
import { sessionScenes } from './SessionScenes.js';
import { sessionNavigation } from './SessionNavigation.js';
import { sessionSubmissions } from './SessionSubmissions.js';
import {
  sessionDigest,
  sessionFile,
  sessionJsonScalars,
  sessionScope,
  sessionUuid,
  utf8Bytes,
} from './SessionPrimitives.js';

/** Match all persisted coordinates to the exact final artifact, including scene and source. */
function selection(
  value: SessionSelection | null,
  outputs: ReadonlyMap<string, SessionOutput>,
): boolean {
  if (value === null) return true;
  const output = outputs.get(value.outputId);
  return (
    !!output &&
    output.sourceId === value.sourceId &&
    (output.scene?.sceneId ?? null) === value.sceneId &&
    output.batchIndex === value.batchIndex &&
    output.listIndex === value.listIndex
  );
}

/** Validate one output receipt independently of file availability or an embedded metadata recipe. */
export function sessionOutput(value: SessionOutput): boolean {
  return (
    [value.id, value.entryId, value.backend, value.runId, value.sourceId].every(sessionUuid) &&
    sessionDigest(value.digest) &&
    (value.assetDigest === null || sessionDigest(value.assetDigest)) &&
    (value.promptId === null || value.promptId.length > 0) &&
    sessionFile(value.file) &&
    (value.file.storage !== 'temp' || value.assetDigest === value.digest) &&
    (value.assetDigest === null || value.assetDigest === value.digest) &&
    (value.scene === null ||
      ([value.scene.groupId, value.scene.sceneId].every(sessionUuid) &&
        value.scene.order < value.scene.count))
  );
}

/** Name the first rule an occurrence breaks, without matching it by path, title or graph contents. */
function entryError(value: SessionEntry, checkpoint: SessionCheckpoint): string | null {
  if (!sessionSubmissions(value, checkpoint.scope)) return 'entry submissions';
  if (!sessionScenes(value)) return 'entry scenes';
  if (!sessionNavigation(value)) return 'entry navigation';
  if (!sessionUuid(value.id)) return 'entry identity';
  if (sessionDocumentError(value.document) !== null) return 'entry document';
  if (utf8Bytes(JSON.stringify(value.view)) > 1048576) return 'entry view size';
  if (value.input !== null && (!value.input.documentId || !sessionDigest(value.input.signature)))
    return 'entry input';
  const sources = new Set<string>();
  for (const source of value.sources) {
    if (
      !sessionUuid(source.id) ||
      sources.has(source.id) ||
      !source.nodeId ||
      !source.executionPath ||
      source.graphRevision > value.document.graphRevision
    )
      return 'entry sources';
    sources.add(source.id);
  }
  const outputs = new Map<string, SessionOutput>();
  const positions = new Set<string>();
  for (const output of value.outputs) {
    const position = JSON.stringify([
      output.runId,
      output.scene?.sceneId ?? null,
      output.sourceId,
      output.listIndex,
      output.batchIndex,
      output.imageIndex,
    ]);
    if (
      !sessionOutput(output) ||
      output.entryId !== value.id ||
      output.backend !== checkpoint.scope.backend ||
      !sources.has(output.sourceId) ||
      outputs.has(output.id) ||
      positions.has(position)
    )
      return 'entry outputs';
    outputs.set(output.id, output);
    positions.add(position);
  }
  if (
    !selection(value.view.selection, outputs) ||
    !selection(value.view.comparison.selection, outputs) ||
    (value.view.comparison.enabled && value.view.comparison.selection === null)
  )
    return 'entry selection';
  const cameras = new Set<string>();
  for (const camera of value.view.cameras) {
    if (!outputs.has(camera.outputId) || cameras.has(camera.outputId)) return 'entry cameras';
    cameras.add(camera.outputId);
  }
  return null;
}

/**
 * Name the first v1 rule a checkpoint breaks, or null when it is admissible.
 *
 * The name identifies a rule, never checkpoint content, so it can appear in a refusal.
 */
export function desktopCheckpointError(value: unknown): string | null {
  if (!isSessionCheckpoint(value)) return 'shape';
  if (!sessionJsonScalars(value)) return 'text';
  if (!sessionScope(value.scope)) return 'scope';
  if (
    !sessionUuid(value.id) ||
    (value.previousId !== null && (!sessionUuid(value.previousId) || value.previousId === value.id))
  )
    return 'identity';
  if (!value.producer || !value.frontend || !value.sugarcubes) return 'versions';
  if (value.placement !== null && sessionPlacementError(value.placement) !== null)
    return 'placement';
  const entries = new Set<string>();
  const outputs = new Set<string>();
  const prompts = new Set<string>();
  let count = 0;
  for (const item of value.entries) {
    if (entries.has(item.id)) return 'duplicate entry';
    const error = entryError(item, value);
    if (error) return error;
    entries.add(item.id);
    count += item.outputs.length;
    for (const submission of item.submissions) {
      if (prompts.has(submission.promptId)) return 'shared prompt';
      prompts.add(submission.promptId);
    }
    for (const output of item.outputs) {
      if (outputs.has(output.id)) return 'shared output';
      outputs.add(output.id);
    }
  }
  if (count > 65536) return 'output count';
  if (value.activeEntry === null ? entries.size !== 0 : !entries.has(value.activeEntry))
    return 'active entry';
  const assets = new Map(value.assets.map((asset) => [asset.digest, asset]));
  if (
    assets.size !== value.assets.length ||
    value.assets.some(
      (asset) =>
        !sessionDigest(asset.digest) ||
        (['mp4', 'webm'].includes(asset.extension)
          ? asset.media !== 'video'
          : asset.media !== 'image'),
    ) ||
    value.assets.reduce((total, asset) => total + asset.bytes, 0) > 8589934592
  )
    return 'assets';
  const referenced = new Set<string>();
  for (const item of value.entries)
    for (const output of item.outputs) {
      if (output.assetDigest === null) continue;
      const asset = assets.get(output.assetDigest);
      if (
        !asset ||
        asset.digest !== output.digest ||
        asset.bytes !== output.bytes ||
        asset.media !== output.media
      )
        return 'asset receipts';
      referenced.add(asset.digest);
    }
  if (referenced.size !== assets.size) return 'unreferenced assets';
  return utf8Bytes(JSON.stringify(value)) <= 134217728 ? null : 'size';
}

/** Validate v1 before storage or materialization; rejected original bytes belong to the store. */
export function isDesktopCheckpoint(value: unknown): value is SessionCheckpoint {
  return desktopCheckpointError(value) === null;
}
