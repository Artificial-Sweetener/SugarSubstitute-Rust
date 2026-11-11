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
/** Workflow scene authority and generation eligibility consume the one core scene parser outside editor input. */
import { requireValue } from '../../../core/src/required.js';
import { parseScenes } from '../../../core/src/index.js';
import type {
  WorkflowSceneAnalysis,
  WorkflowSceneDiagnostics,
  WorkflowSceneGeneration,
  WorkflowSceneInput,
  WorkflowScenePrompt,
} from './contracts.js';
/** Reject open or malformed host records without interpolating source, titles or identities into errors. */
function record(value: unknown, keys: string): Record<string, unknown> {
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    Object.keys(value).sort().join() !== keys
  )
    throw new TypeError('Invalid workflow scene publication');
  return value as Record<string, unknown>;
}
/** Stable host identities and revisions are bounded nonempty strings; their spelling remains literal. */
function identity(value: unknown): string {
  if (typeof value !== 'string' || !value || value.length > 256)
    throw new TypeError('Invalid workflow scene identity');
  return value;
}
/** Capture a closed host snapshot before analysis; aggregate text bounds constrain preparation without truncating scenes. */
function readWorkflow(input: WorkflowSceneInput): WorkflowSceneInput {
  const value = record(input, 'prompts,revision,stackOrder'),
    revision = identity(value.revision);
  if (
    !Array.isArray(value.stackOrder) ||
    value.stackOrder.length > 10000 ||
    !Array.isArray(value.prompts) ||
    value.prompts.length > 10000
  )
    throw new TypeError('Invalid workflow scene lists');
  const stackOrder = Array.from(value.stackOrder, (item: unknown) => identity(item));
  if (new Set(stackOrder).size !== stackOrder.length)
    throw new TypeError('Duplicate workflow stack identity');
  const seen = new Set<string>();
  let total = 0;
  const prompts = Array.from(value.prompts, (item: unknown): WorkflowScenePrompt => {
    const row = record(item, 'id,linkable,role,source,stackId'),
      id = identity(row.id),
      stackId = identity(row.stackId);
    if (
      seen.has(id) ||
      (row.role !== 'positive' && row.role !== 'negative') ||
      typeof row.linkable !== 'boolean' ||
      (row.source !== null && typeof row.source !== 'string')
    )
      throw new TypeError('Invalid workflow prompt endpoint');
    seen.add(id);
    const source = row.source;
    total += source?.length ?? 0;
    if ((source?.length ?? 0) > 1000000 || total > 8000000)
      throw new RangeError('Workflow prompt preparation limit exceeded');
    return Object.freeze({
      id,
      stackId,
      role: row.role,
      linkable: row.linkable,
      source,
    });
  });
  return Object.freeze({
    revision,
    stackOrder: Object.freeze(stackOrder),
    prompts: Object.freeze(prompts),
  });
}
/** Select the first unique linkable positive endpoint in stack order, then diagnose every linkable prompt against its scene identities. */
export function analyzeWorkflowScenes(input: WorkflowSceneInput): WorkflowSceneAnalysis {
  const { revision, stackOrder, prompts } = readWorkflow(input),
    positives = new Map<string, WorkflowScenePrompt | null>();
  for (const prompt of prompts)
    if (prompt.linkable && prompt.role === 'positive')
      positives.set(prompt.stackId, positives.has(prompt.stackId) ? null : prompt);
  let authority: WorkflowScenePrompt | null = null;
  for (const stackId of stackOrder) {
    authority = positives.get(stackId) ?? null;
    if (authority) break;
  }
  const documents = new Map(
      prompts
        .filter((prompt) => prompt.linkable)
        .map((prompt) => [prompt.id, parseScenes(prompt.source ?? '')]),
    ),
    scenes = Object.freeze(
      (authority ? requireValue(documents.get(authority.id), 'workflow').scenes : [])
        .filter((scene) => !scene.marker.duplicate)
        .map(({ marker }, order) =>
          Object.freeze({
            key: marker.normalizedKey,
            title: marker.title,
            order,
          }),
        ),
    ),
    keys = new Set(scenes.map((scene) => scene.key)),
    diagnostics = Object.freeze(
      [...documents].map(([promptId, document]): WorkflowSceneDiagnostics => {
        const orphanKeys = new Set<string>(),
          duplicateKeys = new Set<string>();
        for (const { marker } of document.scenes) {
          if (marker.duplicate) duplicateKeys.add(marker.normalizedKey);
          if (promptId !== authority?.id && !keys.has(marker.normalizedKey))
            orphanKeys.add(marker.normalizedKey);
        }
        return Object.freeze({
          promptId,
          orphanKeys: Object.freeze([...orphanKeys]),
          duplicateKeys: Object.freeze([...duplicateKeys]),
        });
      }),
    ),
    invalid = Object.freeze(
      diagnostics
        .filter((item) => item.orphanKeys.length || item.duplicateKeys.length)
        .map((item) => item.promptId),
    ),
    generation: WorkflowSceneGeneration = Object.freeze(
      !authority
        ? { status: 'no-authority' }
        : !scenes.length
          ? { status: 'no-scenes' }
          : invalid.length
            ? { status: 'invalid-scenes', promptIds: invalid }
            : { status: 'ready' },
    );
  return Object.freeze({
    revision,
    authorityId: authority?.id ?? null,
    scenes,
    diagnostics,
    generation,
  });
}
