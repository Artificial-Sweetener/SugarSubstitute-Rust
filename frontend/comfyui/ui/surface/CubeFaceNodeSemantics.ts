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
/** Classify the narrow node semantics shared with SugarSubstitute Cube cards. */

import type { ComfyNode, ComfyInput, ComfyOutput } from '../types/graph.js';

const HARD_HIDDEN_CLASSES = new Set(['SimpleSyrup.ScheduleAndEncodePromptsWithPromptControl']);
const HOST_OWNED_ACTIVATION_CLASSES = new Set(['VectorscopeCC']);
const NON_RESOURCE_SIGNALS = new Set([
  'ANY',
  'BOOLEAN',
  'COMBO',
  'FLOAT',
  'IMAGE',
  'INT',
  'INTEGER',
  'LATENT',
  'LIST',
  'MASK',
  'NUMBER',
  'STRING',
]);

/** Return whether Substitute treats this infrastructure node as hard-hidden. */
export function isHardHiddenCubeFaceNode(node: ComfyNode): boolean {
  return HARD_HIDDEN_CLASSES.has(nodeClass(node));
}

/** Return whether Substitute suppresses the generic activation switch for a worker. */
export function cubeFaceNodeAllowsActivationControl(node: ComfyNode): boolean {
  if (isHardHiddenCubeFaceNode(node)) return false;
  if (nodeClass(node) === 'KSampler') return false;
  const widgetNames = new Set(
    (node.widgets ?? []).map((widget) => widget.name?.trim().toLowerCase() ?? ''),
  );
  return !(widgetNames.has('steps') && widgetNames.has('denoise'));
}

/** Infer resource/config types that pass through one ordinary graph node. */
export function inferCubeFaceTransformSignals(node: ComfyNode): ReadonlySet<string> {
  if (
    node.isSubgraphNode?.() === true ||
    isHardHiddenCubeFaceNode(node) ||
    HOST_OWNED_ACTIVATION_CLASSES.has(nodeClass(node))
  ) {
    return new Set();
  }
  const inputs = new Set(
    (node.inputs ?? [])
      .map((slot) => resourceSignal(slotType(slot)))
      .filter((signal): signal is string => signal !== null),
  );
  return new Set(
    (node.outputs ?? [])
      .map((slot) => resourceSignal(slotType(slot)))
      .filter((signal): signal is string => signal !== null && inputs.has(signal)),
  );
}

/** Resolve the stable Comfy class identity used by semantic overrides. */
function nodeClass(node: ComfyNode): string {
  return (node.class_type ?? node.type ?? '').trim();
}

/** Read one dynamic slot type without admitting non-string host values. */
function slotType(slot: ComfyInput | ComfyOutput): string | null {
  return typeof slot.type === 'string' ? slot.type : null;
}

/** Normalize one resource/config type and reject ordinary data/widget signals. */
function resourceSignal(value: string | null): string | null {
  const normalized = value?.trim().toUpperCase() ?? '';
  return normalized && !NON_RESOURCE_SIGNALS.has(normalized) ? normalized : null;
}
