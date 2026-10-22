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
/** Delegate popup rendering to a host while native commands retain source and invocation authority. */
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import { contextMenu, type ContextMenu } from './context-menu-state.js';
import type { ContextMenuItem } from './context-menu-contracts.js';

/** Host presentation owns keyboard controls, focus, exclusivity and dismissal. */
export interface PromptMenuPresentation {
  show(
    menu: ContextMenu,
    activate: (item: ContextMenuItem) => Promise<boolean>,
    dismiss: () => void,
  ): void;
  hide(): void;
  dispose(): void;
}

/** Bind one presentation to the actual native editor root. */
export type PromptMenuFactory = (root: HTMLElement) => PromptMenuPresentation;

interface MenuCommands {
  activate(menu: ContextMenu, item: ContextMenuItem): Promise<boolean>;
  close(restoreFocus: boolean): unknown;
}

/** Publish prepared native action identities without installing another menu renderer. */
export function promptMenuPresentation(
  controller: (view: EditorView) => MenuCommands | null,
  create: PromptMenuFactory,
): Extension {
  return ViewPlugin.define((view) => {
    const presentation = create(view.dom);
    let disposed = false;
    let published: ContextMenu | null = null;
    let pending = false;
    /** Reconcile after native dispatch so host focus and dismissal cannot reenter a view update. */
    const reconcile = (): void => {
      if (pending || disposed) return;
      pending = true;
      queueMicrotask(() => {
        pending = false;
        if (disposed) return;
        const menu = view.state.field(contextMenu);
        if (menu === published) return;
        published = menu;
        if (!menu || menu.surface !== 'local') presentation.hide();
        else
          presentation.show(
            menu,
            (item) => controller(view)?.activate(menu, item) ?? Promise.resolve(false),
            () => {
              if (!disposed && view.state.field(contextMenu) === menu)
                controller(view)?.close(false);
            },
          );
      });
    };
    reconcile();
    return {
      /** Native menu state is the sole action-publication source. */
      update(_update: ViewUpdate): void {
        reconcile();
      },
      /** Retire queued work before releasing host presentation. */
      destroy(): void {
        disposed = true;
        presentation.dispose();
      },
    };
  });
}
