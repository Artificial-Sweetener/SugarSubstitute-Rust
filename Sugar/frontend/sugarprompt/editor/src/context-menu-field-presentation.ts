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
/** Aggregate semantic preparation and literal projection delegate every lifetime transition to the sole native invocation owner. */
import type { EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import type { ContextMenuItem } from './context-menu-contracts.js';
import { contextMenuTargetAt, prepareContextMenu } from './context-menu-preparation.js';
import {
  contextMenu,
  currentContextMenu,
  type ContextMenu,
  type PreparedContextMenu,
} from './context-menu-state.js';
import type {
  FieldActionContext,
  FieldActionOutcome,
  FieldActionPreparation,
} from './field-actions-contracts.js';
import { projectFieldActions } from './field-actions-projection.js';

/** This is the same issued promise settlement retained and retired by NativeContextMenu. */
export interface FieldMenuSettlement {
  /** Publish only the issued preparation's typed result. */
  resolve(outcome: FieldActionPreparation): void;
  /** Preserve unexpected primary and retirement causes on the issued promise. */
  reject(cause: unknown): void;
}

/** Presentation owns no generation, focus capability, selection admission, edit registry or request lane. */
export interface FieldMenuPublication {
  /** Ask the original native capture/issued-generation authority before each publication stage. */
  current(): boolean;
  /** Admit genuine prepared leaves through the sole native edit registry. */
  prepare(state: EditorState, menu: PreparedContextMenu): boolean;
  /** Perform the actual source-neutral publication and opening transition. */
  publish(menu: ContextMenu): boolean;
  /** Detach only this settlement, preserving any newer invocation. */
  detach(): void;
  /** Cancel only this issued invocation or settle its already retired promise. */
  retire(): void;
  /** Attempt failure retirement only while this generation remains issued. */
  fail(): void;
  /** Delegate a genuine projected leaf to the mounted admission and activation owner. */
  activate(menu: ContextMenu, item: ContextMenuItem): Promise<FieldActionOutcome>;
  /** Release only the exact still-installed contribution. */
  release(menu: ContextMenu): void;
}

/** Prepare one finite immutable contribution outside input; source/caret/focus and retirement remain NativeContextMenu-owned. */
export function requestFieldMenu(
  view: EditorView,
  anchor: FieldActionContext,
  position: number,
  settlement: FieldMenuSettlement,
  owner: FieldMenuPublication,
): void {
  queueMicrotask(() => {
    try {
      if (!owner.current()) {
        owner.retire();
        return;
      }
      const state = view.state,
        target = contextMenuTargetAt(state, position);
      const prepared = target ? prepareContextMenu(state, target, 'field') : null;
      if (!prepared || !currentContextMenu(state, prepared) || !owner.prepare(state, prepared)) {
        owner.retire();
        return;
      }
      const menu: ContextMenu = Object.freeze({ ...prepared, anchor });
      if (
        !owner.current() ||
        !owner.publish(menu) ||
        !owner.current() ||
        view.state.field(contextMenu) !== menu
      ) {
        owner.retire();
        return;
      }
      owner.detach();
      settlement.resolve({
        status: 'ready',
        contribution: projectFieldActions(
          menu.items,
          (item) => owner.activate(menu, item),
          () => owner.release(menu),
        ),
      });
    } catch (cause) {
      owner.detach();
      try {
        owner.fail();
      } catch (cleanup) {
        settlement.reject(
          new AggregateError([cause, cleanup], 'field-action-preparation-retirement-failed'),
        );
        return;
      }
      settlement.reject(cause);
    }
  });
}
