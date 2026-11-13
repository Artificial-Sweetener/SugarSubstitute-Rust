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
/** Native scheduling publications bind service-prepared models to current source, resources and host authority. */
import { Facet, StateEffect, StateField, type EditorState } from '@codemirror/state';
import type { PreparedLoraSchedulingContext } from '../../services/src/index.js';
import type { PreparedDocumentLoraSchedules } from '../../services/src/lora/scene-schedules.js';
import { assertPreparedLoraSchedulingContext } from '../../services/src/lora/scheduled-context.js';
import { featureEnabled } from '../../services/src/index.js';
import { featureProfile } from './features.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { loraPresentation, type LoraPresentation } from './lora-presentation.js';

/** A compatible host publication names the resource owner's accepted catalog revision. */
export interface LoraSchedulingHost {
  readonly context: PreparedLoraSchedulingContext | null;
  readonly catalogRevision: string | null;
}

/** Capture one immutable host envelope; prepared context metadata is already service-owned and frozen. */
export function captureLoraSchedulingHost(host: LoraSchedulingHost): LoraSchedulingHost {
  if (
    !host ||
    typeof host !== 'object' ||
    Array.isArray(host) ||
    Object.keys(host).sort().join() !== 'catalogRevision,context'
  )
    throw new TypeError('Invalid scheduling host');
  if (host.context !== null) assertPreparedLoraSchedulingContext(host.context);
  if (
    host.catalogRevision !== null &&
    (typeof host.catalogRevision !== 'string' ||
      !host.catalogRevision.trim() ||
      host.catalogRevision.length > 256)
  )
    throw new TypeError('Invalid scheduling catalog revision');
  return Object.freeze({
    context: host.context,
    catalogRevision: host.catalogRevision,
  });
}

/** Competing scheduling authorities cannot populate the same editor. */
export const loraSchedulingHost = Facet.define<LoraSchedulingHost, LoraSchedulingHost>({
  /** An absent host remains explicitly unavailable. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple LoRA scheduling hosts');
    return values[0] ?? Object.freeze({ context: null, catalogRevision: null });
  },
});

/** The service owns effective models; this envelope proves native publication eligibility. */
export interface NativeLoraSchedule {
  readonly freshness: EditorFreshness;
  readonly host: LoraSchedulingHost;
  readonly resources: LoraPresentation;
  /** Source-bound scoped preparation is consumed outside input for caret and menu-target lookup. */
  readonly scopes: PreparedDocumentLoraSchedules;
  /** Prepared logical-document UTF-16 lookup scopes menu eligibility to its target, including canonical inline identity. */
  readonly hasTriggersAt: (position: number, backendKey?: string) => boolean;
}

/** Prepared output never publishes after source, permission, catalog or host supersession. */
export const installLoraSchedule = StateEffect.define<NativeLoraSchedule>();

/** Compare authoritative identities without parsing or traversing model catalogs. */
function current(state: EditorState, value: NativeLoraSchedule): boolean {
  return (
    featureEnabled(state.facet(featureProfile), 'lora_trigger_words') &&
    value.freshness === state.field(editorFreshness) &&
    value.host === state.facet(loraSchedulingHost) &&
    value.resources === state.field(loraPresentation)
  );
}

/** Selection reuses the actual schedule; source/context/resource replacement retires it immediately. */
export const nativeLoraSchedule = StateField.define<NativeLoraSchedule | null>({
  /** Baselines await finite off-input preparation. */
  create() {
    return null;
  },
  /** Accept only publications that still match every native dependency. */
  update(value, transaction) {
    if (value && !current(transaction.state, value)) value = null;
    for (const effect of transaction.effects)
      if (effect.is(installLoraSchedule) && current(transaction.state, effect.value))
        value = effect.value;
    return value;
  },
});
