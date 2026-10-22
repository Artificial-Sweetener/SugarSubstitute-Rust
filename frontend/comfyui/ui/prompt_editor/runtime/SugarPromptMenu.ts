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
/** Present SugarPrompt's prepared actions through the shared SugarCubes context menu owner. */
import { ContextMenu } from '../../menus/ContextMenu.js';
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import type { ContextMenu as PromptMenu } from '../../../../sugarprompt/editor/src/context-menu-state.js';
import type { ContextMenuItem } from '../../../../sugarprompt/editor/src/context-menu-contracts.js';
import type { PromptMenuPresentation } from '../../../../sugarprompt/editor/src/menu-presentation.js';

/** Native action identities remain captured while shared controls own all popup interaction. */
export class SugarPromptMenu implements PromptMenuPresentation {
  readonly #menu: ContextMenu;
  readonly #root: HTMLElement;
  readonly #report: (cause: unknown) => void;
  #publication: object | null = null;

  /** Bind focus return and observable command failures to one editor lifetime. */
  constructor(root: HTMLElement, report: (cause: unknown) => void) {
    this.#root = root;
    this.#report = report;
    this.#menu = new ContextMenu(root.ownerDocument);
  }

  /** Map literal domain entries without changing source or reconstructing commands. */
  show(
    menu: PromptMenu,
    activate: (item: ContextMenuItem) => Promise<boolean>,
    dismiss: () => void,
  ): void {
    this.hide();
    const publication = {};
    this.#publication = publication;
    const entries = (items: readonly ContextMenuItem[]): ContextMenuEntry[] =>
      items.map((item) => {
        if (item.kind === 'separator') return { label: '', separator: true };
        if (item.kind === 'submenu') return { label: item.label, children: entries(item.children) };
        if (item.kind === 'heading') return { label: item.label, section: true };
        if (item.kind === 'disabled') return { label: item.label, disabled: true };
        return {
          label: item.label,
          ...(item.kind === 'rich-rendering'
            ? { checked: item.checked, selection: 'multiple' as const }
            : {}),
          select: () => {
            void activate(item).catch(this.#report);
          },
        };
      });
    this.#menu.open({
      label: 'Prompt actions',
      entries: entries(menu.items),
      anchor: { kind: 'pointer', x: menu.anchor.left, y: menu.anchor.bottom },
      focusReturn: this.#root.querySelector<HTMLElement>('.cm-content'),
      returnFocusOnSelect: false,
      attributes: { 'data-sc-prompt-menu': '' },
      closed: () =>
        queueMicrotask(() => {
          if (this.#publication === publication) dismiss();
        }),
    });
  }

  /** Clear presentation without retiring a replacement native invocation. */
  hide(): void {
    this.#publication = null;
    this.#menu.close();
  }

  /** Release the shared menu before the editor's DOM is removed. */
  dispose(): void {
    this.hide();
  }
}
