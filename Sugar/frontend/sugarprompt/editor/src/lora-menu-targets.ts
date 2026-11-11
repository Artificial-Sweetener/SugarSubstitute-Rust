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
/** LoRA menu target policy consumes source-current chips and prepared host intents without owning native overlay state. */
import type { EditorState } from '@codemirror/state';
import type {
  LoraPageRequest,
  LoraMetadataMenuItem,
  LoraResource,
} from '../../services/src/index.js';
import {
  loraPageHost,
  loraMetadataHost,
  type LoraPageHost,
  type LoraMetadataHost,
} from './lora-action-hosts.js';
import { loraMetadataActions } from './lora-metadata-actions.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { loraPresentation } from './lora-presentation.js';
import { richProjection } from './rich-projection.js';
import { LoraWidget } from './lora-widget.js';
import { nativeLoraSchedule, type NativeLoraSchedule } from './lora-schedule.js';
import type { ContextMenuItem } from './context-menu-contracts.js';
import { loraNameKey } from '../../services/src/lora/names.js';

/** Prepared logical-document UTF-16 bounds and canonical external intent; the original authored name is only an index key. */
export interface LoraModelTarget {
  readonly from: number;
  readonly to: number;
  readonly name: string;
  readonly resource: LoraResource;
  readonly page: LoraPageRequest | null;
  readonly metadata: readonly LoraMetadataMenuItem[];
  readonly metadataLeaves: ReadonlySet<LoraMetadataMenuItem>;
}

/** Prepared LoRA acceptance binds action identities to the current source, profile, metadata and host. */
export interface LoraMenuContext {
  readonly freshness: EditorFreshness;
  readonly pageHost: LoraPageHost | null;
  readonly metadataHost: LoraMetadataHost | null;
  readonly items: readonly ContextMenuItem[];
  readonly pageItem: Extract<ContextMenuItem, { kind: 'page' }> | null;
  readonly schedule: NativeLoraSchedule | null;
  readonly editLeaves: ReadonlySet<Extract<ContextMenuItem, { kind: 'source-edit' }>>;
}

/** Inline actions retain the exact rendered model target; general actions use a captured logical-document UTF-16 position. */
export type PreparedLoraMenu = LoraMenuContext &
  (
    | (LoraModelTarget & { readonly kind: 'inline' })
    | { readonly kind: 'general'; readonly from: number; readonly to: number }
  );

/** Query actual rendered decorations only, preserving raw/revealed syntax and refusing stale projections. */
export function loraModelTargetAt(state: EditorState, position: number): LoraModelTarget | null {
  const projection = state.field(richProjection),
    presentation = state.field(loraPresentation);
  if (
    projection.sourceIdentity !== state.field(editorFreshness).source ||
    projection.loraPresentation !== presentation
  )
    return null;
  const actions = state.field(loraMetadataActions, false);
  let target: LoraModelTarget | null = null;
  projection.decorations.between(position, position, (from, to, decoration) => {
    const widget = decoration.spec.widget;
    if (!(widget instanceof LoraWidget)) return;
    const name = widget.resource.name,
      page = presentation.pages.get(name) ?? null;
    const prepared =
      actions?.presentation === presentation && actions.host === state.facet(loraMetadataHost)
        ? actions.byName.get(name)
        : undefined;
    const metadata = prepared?.items ?? [];
    if (
      ((page && state.facet(loraPageHost)) ||
        (!state.readOnly &&
          widget.resource.resolution === 'found' &&
          state
            .field(nativeLoraSchedule, false)
            ?.hasTriggersAt(position, loraNameKey(widget.resource.model.backendValue))) ||
        (metadata.length && state.facet(loraMetadataHost))) &&
      (!target || from === position)
    )
      target = {
        from,
        to,
        name,
        resource: widget.resource,
        page,
        metadata,
        metadataLeaves: prepared?.leaves ?? new Set(),
      };
  });
  return target;
}

/** Accepted external work follows canonical source/context/host identities independently of caret-driven folding. */
export function currentLoraTarget(state: EditorState, menu: PreparedLoraMenu): boolean {
  return (
    menu.kind === 'inline' &&
    state.field(editorFreshness) === menu.freshness &&
    state.facet(loraPageHost) === menu.pageHost &&
    state.facet(loraMetadataHost) === menu.metadataHost &&
    state.field(loraPresentation).byName.get(menu.name) === menu.resource
  );
}

/** Visible menu acceptance additionally requires the actual current source-backed chip. */
export function currentLoraMenu(state: EditorState, menu: PreparedLoraMenu): boolean {
  return (
    state.field(editorFreshness) === menu.freshness &&
    state.facet(loraPageHost) === menu.pageHost &&
    state.facet(loraMetadataHost) === menu.metadataHost &&
    (!menu.editLeaves.size ||
      (!state.readOnly && state.field(nativeLoraSchedule, false) === menu.schedule)) &&
    (menu.kind === 'general' ||
      (currentLoraTarget(state, menu) &&
        loraModelTargetAt(state, menu.from)?.resource === menu.resource))
  );
}
