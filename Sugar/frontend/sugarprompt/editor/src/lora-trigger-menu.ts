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
/** Canonical scope admission and core insertion policy prepare genuine encoded trigger menu alternatives. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState } from '@codemirror/state';
import { prepareTriggerInsertionSlot, triggerInsertionText } from '../../core/src/index.js';
import { nativeLoraSchedule, type NativeLoraSchedule } from './lora-schedule.js';
import { nativeDocumentValueAt } from './document-values.js';
import { prepareDocumentValueEdit } from './document-value-edit-issuance.js';
import { semanticStructures } from './structure.js';
import type { ContextMenuItem } from './context-menu-contracts.js';
import type { LoraModelTarget } from './lora-menu-targets.js';
import { loraNameKey } from '../../services/src/lora/names.js';
/** Actual literal leaves carry genuine edits and retain one captured native schedule authority. */
export interface PreparedTriggerMenu {
  readonly schedule: NativeLoraSchedule | null;
  readonly items: readonly ContextMenuItem[];
  readonly leaves: ReadonlySet<
    Extract<
      ContextMenuItem,
      {
        kind: 'source-edit';
      }
    >
  >;
}
/** General selection must remain in one scope; inline append uses the actual whole chip's admitted scope. */
export function prepareLoraTriggerMenu(
  state: EditorState,
  position: number,
  inline: LoraModelTarget | null,
): PreparedTriggerMenu {
  const schedule = state.field(nativeLoraSchedule, false) ?? null,
    semantic = state.field(semanticStructures).native,
    leaves = new Set<
      Extract<
        ContextMenuItem,
        {
          kind: 'source-edit';
        }
      >
    >();
  if (state.readOnly || !schedule || !semantic) return { schedule: null, items: [], leaves };
  const selection = state.selection.main,
    context = {
      values: semantic.nativeValues,
      semantics: semantic.documentSemantics,
      enabled: true,
      composing: false,
    },
    admitted = nativeDocumentValueAt(
      state,
      context,
      inline
        ? { from: inline.from, to: inline.to }
        : !selection.empty
          ? { from: selection.from, to: selection.to }
          : { from: position, to: position },
    );
  if (admitted.status !== 'ready') return { schedule: null, items: [], leaves };
  const eligibility = inline
    ? admitted
    : nativeDocumentValueAt(state, context, {
        from: position,
        to: position,
      });
  if (eligibility.status !== 'ready') return { schedule: null, items: [], leaves };
  const target = admitted.target,
    clicked = eligibility.target,
    exact = requireValue(
      clicked.scope.mapping.decodedToSource[clicked.decodedRange.from],
      'lora-trigger-menu',
    ),
    scoped = schedule.scopes.at(exact);
  if (!scoped || scoped.snapshot.status !== 'ready') return { schedule: null, items: [], leaves };
  const slot = prepareTriggerInsertionSlot(
    target.scope.mapping.text,
    !inline && !selection.empty
      ? { kind: 'selection', range: target.decodedRange }
      : {
          kind: 'position',
          position: inline ? target.decodedRange.to : target.decodedRange.from,
        },
  );
  if (!slot) return { schedule: null, items: [], leaves };
  const inlineModel =
    inline?.resource.resolution === 'found'
      ? scoped.insertions.get(loraNameKey(inline.resource.model.backendValue))
      : undefined;
  for (const model of inline ? (inlineModel ? [inlineModel] : []) : scoped.insertions.values()) {
    const insertion = triggerInsertionText(slot, model.insertionText);
    if (!insertion) continue;
    const caret = slot.range.from + insertion.length,
      outcome = prepareDocumentValueEdit(state, {
        context,
        target,
        range: slot.range,
        text: insertion,
        selection: {
          ranges: [{ kind: 'decoded', anchor: caret, head: caret }],
          mainIndex: 0,
        },
        publication: { purpose: 'trigger' },
      });
    if (outcome.status !== 'ready') continue;
    leaves.add(
      Object.freeze({
        kind: 'source-edit',
        purpose: 'trigger',
        label: model.display,
        title: `Trigger words: ${model.display}`,
        replacement: outcome.edit,
      }),
    );
  }
  const children = Object.freeze([...leaves]);
  return {
    schedule,
    leaves,
    items: inline
      ? children
      : children.length
        ? [
            Object.freeze({
              kind: 'submenu',
              label: 'Insert trigger words',
              children,
            }),
          ]
        : [],
  };
}
