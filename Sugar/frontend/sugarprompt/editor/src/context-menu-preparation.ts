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
/** Compose domain presentation in one native overlay without reproducing source, host or freshness rules. */
import {
  spellingMenuTargetAt,
  prepareSpellingMenu,
  type SpellingMenuTarget,
} from './spelling-menu.js';
import type { PreparedSpellingSuggestions } from '../../services/src/diagnostics/spelling-contracts.js';
import {
  duplicateMenuTargetAt,
  prepareDuplicateMenu,
  type DuplicateMenuTarget,
} from './duplicate-menu.js';
import type { EditorState } from '@codemirror/state';
import { prepareSavedSegmentMenu } from './saved-segment-menu.js';
import { richRendering } from './presentation.js';
import type { PreparedContextMenu } from './context-menu-state.js';
import { prepareLoraMenu } from './lora-menu-preparation.js';
import { loraModelTargetAt, type LoraModelTarget } from './lora-menu-targets.js';
import {
  prepareWildcardMenu,
  wildcardMenuTargetAt,
  type WildcardMenuTarget,
} from './wildcard-menu-target.js';
import { sceneQueueTargetAt } from './scene-queue.js';
import type { ContextMenuItem } from './context-menu-contracts.js';
import { prepareTextMenu } from './text-menu.js';
import { fieldActionUtilities } from './field-action-utilities.js';
import { featureProfile } from './features.js';
import { featureEnabled } from '../../services/src/index.js';
import {
  documentMenuTargetAt,
  prepareDocumentMenu,
  type DocumentMenuTarget,
} from './document-menu-target.js';

/** Captured logical-document UTF-16 position preserves right-button selection and independently prepared targets. */
export interface MenuTarget {
  readonly position: number;
  readonly inline: LoraModelTarget | null;
  readonly diagnostic: WildcardMenuTarget | null;
  readonly documentDiagnostic: DocumentMenuTarget | null;
  readonly duplicateDiagnostic: DuplicateMenuTarget | null;
  readonly spellingDiagnostic: SpellingMenuTarget | null;
}
/** Bounded input eligibility consumes prepared identities only. */
export function contextMenuTargetAt(state: EditorState, position: number): MenuTarget | null {
  const duplicateDiagnostic = duplicateMenuTargetAt(state, position);
  const inline = loraModelTargetAt(state, position),
    diagnostic = inline ? null : wildcardMenuTargetAt(state, position),
    documentDiagnostic = documentMenuTargetAt(state, position),
    spellingDiagnostic = spellingMenuTargetAt(state, position);
  return position >= 0 && position <= state.doc.length
    ? {
        position,
        inline,
        diagnostic,
        documentDiagnostic,
        duplicateDiagnostic,
        spellingDiagnostic,
      }
    : null;
}
/** Diagnostic, standard text, scene and LoRA groups retain independent domain identities through composition and acceptance. */
export function prepareContextMenu(
  state: EditorState,
  target: MenuTarget,
  surface: 'local' | 'field',
  clipboard = { readable: false, writable: false },
  spellingSuggestions: PreparedSpellingSuggestions | null = null,
  spellingSettlement: 'pending' | 'unavailable' = 'pending',
): PreparedContextMenu | null {
  const actions =
      state.readOnly && surface === 'field'
        ? null
        : prepareLoraMenu(state, target.position, surface === 'field' ? null : target.inline),
    diagnostic =
      state.readOnly && surface === 'field'
        ? null
        : target.documentDiagnostic
          ? prepareDocumentMenu(state, target.documentDiagnostic)
          : target.duplicateDiagnostic
            ? prepareDuplicateMenu(state, target.duplicateDiagnostic)
            : target.spellingDiagnostic
              ? prepareSpellingMenu(
                  state,
                  target.spellingDiagnostic,
                  spellingSuggestions,
                  spellingSettlement,
                )
              : target.diagnostic
                ? prepareWildcardMenu(state, target.diagnostic)
                : null,
    queue =
      state.readOnly && surface === 'field' ? null : sceneQueueTargetAt(state, target.position),
    queueItem: PreparedContextMenu['queueItem'] = queue
      ? Object.freeze({
          kind: 'queue-scene',
          label: 'Queue this scene',
          request: queue.request,
        })
      : null;
  const text = prepareTextMenu(state, clipboard);
  const presets = prepareSavedSegmentMenu(state, target.position);
  const utilities = state.facet(fieldActionUtilities);
  const scheduleItem: PreparedContextMenu['scheduleItem'] =
    surface === 'field' &&
    !state.readOnly &&
    utilities &&
    featureEnabled(state.facet(featureProfile), 'lora_picker')
      ? Object.freeze({ kind: 'schedule-lora', label: 'Schedule LoRA…' })
      : null;
  const renderingItem: PreparedContextMenu['renderingItem'] = Object.freeze({
    kind: 'rich-rendering',
    label: 'Rich prompt rendering',
    checked: state.facet(richRendering),
  });
  const items: ContextMenuItem[] = [];
  const insertion = presets.items[1];
  const fieldUtilities: ContextMenuItem[] = [
    ...(presets.saveLeaf ? [presets.saveLeaf] : []),
    ...(scheduleItem ? [scheduleItem] : []),
  ];
  const mutations: ContextMenuItem[] = [
    ...(queueItem ? [queueItem] : []),
    ...(insertion?.kind === 'submenu' ? [insertion] : []),
    ...(actions?.items ?? []),
  ];
  if (mutations.length && fieldUtilities.length) mutations.push({ kind: 'separator' });
  mutations.push(...fieldUtilities);
  for (const group of surface === 'field'
    ? [diagnostic?.items ?? [], mutations, [renderingItem]]
    : [
        diagnostic?.items ?? [],
        text.items,
        presets.items,
        queueItem ? [queueItem] : [],
        actions?.items ?? [],
        actions?.kind === 'inline' ? [] : [renderingItem],
      ]) {
    if (!group.length) continue;
    if (items.length) items.push({ kind: 'separator' });
    items.push(...group);
  }
  return items.length
    ? {
        surface,
        utilities,
        scheduleItem,
        text,
        presets,
        kind: diagnostic?.kind ?? actions?.kind ?? 'general',
        from: diagnostic?.from ?? actions?.from ?? target.position,
        to: diagnostic?.to ?? actions?.to ?? target.position,
        diagnostic,
        actions,
        queue,
        queueItem,
        renderingItem,
        items: Object.freeze(items),
      }
    : null;
}
