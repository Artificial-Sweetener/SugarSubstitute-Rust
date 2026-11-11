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
/** Source-bound prepared action indexes are built outside input and retire with actual metadata/capability identities. */
import { StateEffect, StateField, type EditorState } from '@codemirror/state';
import { prepareLoraMetadataMenu, type LoraMetadataMenuItem } from '../../services/src/index.js';
import { loraMetadataHost, type LoraMetadataHost } from './lora-action-hosts.js';
import { editorFreshness } from './freshness.js';
import { loraPresentation, type LoraPresentation } from './lora-presentation.js';

/** Derived index retains actual resource and capability publications, never another authoritative catalog. */
export interface LoraMetadataActions {
  readonly presentation: LoraPresentation;
  readonly host: LoraMetadataHost | null;
  readonly byName: ReadonlyMap<string, PreparedLoraMetadataMenu>;
}

/** Prepared leaf membership keeps native acceptance constant-time without traversing the visible hierarchy. */
export interface PreparedLoraMetadataMenu {
  readonly items: readonly LoraMetadataMenuItem[];
  readonly leaves: ReadonlySet<LoraMetadataMenuItem>;
}

/** Collect leaf object identities once with external preparation, preserving actual immutable output reuse. */
function preparedMenu(items: readonly LoraMetadataMenuItem[]): PreparedLoraMetadataMenu {
  const leaves = new Set<LoraMetadataMenuItem>();
  /** Recursive traversal belongs to prepared tree construction, never native input. */
  function collect(item: LoraMetadataMenuItem): void {
    if (item.kind === 'action') leaves.add(item);
    else if (item.kind === 'submenu') item.children.forEach(collect);
  }
  items.forEach(collect);
  return { items, leaves };
}

/** Build canonical menu requests when a resource or host publishes, before native input/paint consumes them. */
export function prepareLoraMetadataActions(
  state: EditorState,
  presentation = state.field(loraPresentation),
): LoraMetadataActions {
  const host = state.facet(loraMetadataHost),
    byName = new Map<string, PreparedLoraMetadataMenu>();
  if (host && presentation.sourceIdentity === state.field(editorFreshness).source)
    for (const resource of presentation.snapshot.resources)
      if (resource.resolution === 'found')
        byName.set(
          resource.name,
          preparedMenu(
            prepareLoraMetadataMenu(resource.model, host.catalogRevision, host.capability),
          ),
        );
  return { presentation, host, byName };
}

/** Publication is explicit outside native input; current-state checks reject stale candidates. */
export const installLoraMetadataActions = StateEffect.define<LoraMetadataActions>();

/** Selection and presentation changes reuse actual prepared trees; source/context changes make old data ineligible at consumption. */
export const loraMetadataActions = StateField.define<LoraMetadataActions>({
  create: (state) => ({
    presentation: state.field(loraPresentation),
    host: state.facet(loraMetadataHost),
    byName: new Map(),
  }),
  update(current, transaction) {
    for (const effect of transaction.effects)
      if (
        effect.is(installLoraMetadataActions) &&
        effect.value.presentation === transaction.state.field(loraPresentation) &&
        effect.value.host === transaction.state.facet(loraMetadataHost)
      )
        current = effect.value;
    return current;
  },
});
