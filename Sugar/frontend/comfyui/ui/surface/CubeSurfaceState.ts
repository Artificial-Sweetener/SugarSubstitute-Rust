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
/** Own validation and serialization of persisted Cube-face presentation state. */

import { isRecord } from '../types/common.js';

const SURFACE_SCHEMA = 3;
const DEFAULT_MINIMUM_COLUMN_WIDTH = 240;
const DEFAULT_GAP = 12;
const DEFAULT_PREVIEW_WIDTH = 320;

export interface CubePreviewState {
  visible: boolean;
  width: number;
  selectedOutput: string | null;
}

export interface CubeFaceCardState {
  authoredBypass: boolean;
  revealed: boolean;
  enabledOverride: boolean | null;
  activeMode: number;
}

export interface CubeSurfaceState {
  schema: 3;
  nodeOrder: string[];
  cards: Record<string, CubeFaceCardState>;
  minimumColumnWidth: number;
  gap: number;
  preview: CubePreviewState;
}

export interface SerializedCubeSurfaceState {
  schema: 3;
  node_order: string[];
  cards: Record<
    string,
    {
      authored_bypass: boolean;
      revealed: boolean;
      enabled_override: boolean | null;
      active_mode: number;
    }
  >;
  minimum_column_width: number;
  gap: number;
  preview: {
    visible: boolean;
    width: number;
    selected_output: string | null;
  };
}

/** Create the authoritative defaults used when opening a legacy Cube instance. */
export function createDefaultCubeSurfaceState(): CubeSurfaceState {
  return {
    schema: SURFACE_SCHEMA,
    nodeOrder: [],
    cards: {},
    minimumColumnWidth: DEFAULT_MINIMUM_COLUMN_WIDTH,
    gap: DEFAULT_GAP,
    preview: {
      visible: true,
      width: DEFAULT_PREVIEW_WIDTH,
      selectedOutput: null,
    },
  };
}

/** Validate untrusted persisted state without allowing invalid geometry into the UI. */
export function parseCubeSurfaceState(value: unknown): CubeSurfaceState {
  const defaults = createDefaultCubeSurfaceState();
  if (!isRecord(value)) return defaults;
  const preview = isRecord(value.preview) ? value.preview : {};

  return {
    schema: SURFACE_SCHEMA,
    nodeOrder: uniqueStrings(value.node_order),
    cards: parseCardStates(value.cards),
    minimumColumnWidth: finiteNonNegative(value.minimum_column_width, defaults.minimumColumnWidth),
    gap: finiteNonNegative(value.gap, defaults.gap),
    preview: {
      visible: typeof preview.visible === 'boolean' ? preview.visible : defaults.preview.visible,
      width: finiteNonNegative(preview.width, defaults.preview.width),
      selectedOutput:
        typeof preview.selected_output === 'string' && preview.selected_output.trim()
          ? preview.selected_output.trim()
          : null,
    },
  };
}

/** Serialize Cube-face state using stable snake-case host data keys. */
export function serializeCubeSurfaceState(state: CubeSurfaceState): SerializedCubeSurfaceState {
  return {
    schema: SURFACE_SCHEMA,
    node_order: [...state.nodeOrder],
    cards: Object.fromEntries(
      Object.entries(state.cards).map(([nodeId, card]) => [
        nodeId,
        {
          authored_bypass: card.authoredBypass,
          revealed: card.revealed,
          enabled_override: card.enabledOverride,
          active_mode: card.activeMode,
        },
      ]),
    ),
    minimum_column_width: state.minimumColumnWidth,
    gap: state.gap,
    preview: {
      visible: state.preview.visible,
      width: state.preview.width,
      selected_output: state.preview.selectedOutput,
    },
  };
}

/** Validate persisted per-card visibility and activation presentation state. */
function parseCardStates(value: unknown): Record<string, CubeFaceCardState> {
  if (!isRecord(value)) return {};
  const cards: Record<string, CubeFaceCardState> = {};
  for (const [nodeId, candidate] of Object.entries(value)) {
    if (!nodeId.trim() || !isRecord(candidate)) continue;
    const parsed = parseCurrentCardState(candidate) ?? migrateLegacyCardState(candidate);
    if (parsed) cards[nodeId] = parsed;
  }
  return cards;
}

/** Validate one current reveal and activation state record. */
function parseCurrentCardState(candidate: Record<string, unknown>): CubeFaceCardState | null {
  if (
    typeof candidate.authored_bypass !== 'boolean' ||
    typeof candidate.revealed !== 'boolean' ||
    (candidate.enabled_override !== null && typeof candidate.enabled_override !== 'boolean')
  ) {
    return null;
  }
  return {
    authoredBypass: candidate.authored_bypass,
    revealed: candidate.authored_bypass && candidate.revealed,
    enabledOverride: candidate.enabled_override,
    activeMode: finiteMode(candidate.active_mode),
  };
}

/** Migrate schema-two visibility without preserving arbitrary ordinary-card hiding. */
function migrateLegacyCardState(candidate: Record<string, unknown>): CubeFaceCardState | null {
  if (typeof candidate.visible !== 'boolean') return null;
  const revealed = candidate.visible;
  return {
    authoredBypass: revealed && candidate.activation_control !== true,
    revealed,
    enabledOverride: null,
    activeMode: finiteMode(candidate.active_mode),
  };
}

/** Return unique non-empty string identifiers in persisted order. */
function uniqueStrings(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const values: string[] = [];
  for (const entry of value) {
    if (typeof entry !== 'string') continue;
    const normalized = entry.trim();
    if (!normalized || seen.has(normalized)) continue;
    seen.add(normalized);
    values.push(normalized);
  }
  return values;
}

/** Validate a finite non-negative persistence number or use its product default. */
function finiteNonNegative(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? Math.max(0, value) : fallback;
}

/** Retain only executable non-bypass LiteGraph modes for later activation. */
function finiteMode(value: unknown): number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0 && value !== 4
    ? value
    : 0;
}
