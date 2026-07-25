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
/** Compose Cube-owned header actions without owning popup geometry. */

import type { CubeFaceCardMenuEntry } from './CubeFaceCardPolicy.js';
import {
  dispatchCubeFaceTitlebarAction,
  resolveCubeFaceTitlebarActions,
  type CubeFaceChromeActions,
  type CubeFaceChromeMetadata,
  type CubeFaceTitlebarActionKey,
} from './CubeFaceChromeActions.js';
import { CubeFaceRevealMenuView } from './CubeFaceRevealMenuView.js';
import { createComfyPrimeIconElement } from './ComfyPrimeIcons.js';

/** Own the accessible Cube header controls without owning card policy. */
export class CubeFaceActionsView {
  readonly element: HTMLDivElement;
  readonly #revealMenu: CubeFaceRevealMenuView;
  readonly #chromeButtons: Map<CubeFaceTitlebarActionKey, HTMLButtonElement>;
  readonly #metadata: CubeFaceChromeMetadata;
  readonly #chromeActions: CubeFaceChromeActions | null;

  /** Build stable actions that remain mounted while menu entries change. */
  constructor(
    documentRef: Document,
    metadata: CubeFaceChromeMetadata,
    chromeActions: CubeFaceChromeActions | null | undefined,
  ) {
    this.element = documentRef.createElement('div');
    this.element.className = 'sugarcubes-cube-face__actions';
    this.#metadata = metadata;
    this.#chromeActions = chromeActions ?? null;
    this.#chromeButtons = new Map(
      (['swap-left', 'swap-right', 'cube-menu'] as const).map((key) => {
        const button = createChromeButton(documentRef, key);
        button.addEventListener('click', (event) => {
          if (dispatchCubeFaceTitlebarAction(key, this.#metadata, this.#chromeActions, event)) {
            event.preventDefault();
            event.stopPropagation();
          }
        });
        return [key, button];
      }),
    );
    this.#revealMenu = new CubeFaceRevealMenuView(documentRef);
    this.element.append(...this.#chromeButtons.values(), this.#revealMenu.button);
  }

  /** Replace menu rows from the current shared card decisions. */
  render(
    entries: readonly CubeFaceCardMenuEntry[],
    onRevealChange: (nodeId: string, revealed: boolean) => void,
  ): void {
    const visibleActions = resolveCubeFaceTitlebarActions(this.#metadata, this.#chromeActions);
    for (const button of this.#chromeButtons.values()) button.hidden = true;
    for (const action of visibleActions) {
      const button = this.#chromeButtons.get(action.key);
      if (!button) continue;
      button.hidden = false;
      button.replaceChildren(createComfyPrimeIconElement(button.ownerDocument, action.icon));
      button.title = action.title;
      button.setAttribute('aria-label', action.ariaLabel);
    }
    this.#revealMenu.render(entries, onRevealChange);
  }

  /** Close transient menu state when the owning Cube view is released. */
  dispose(): void {
    this.#revealMenu.dispose();
  }
}

/** Build one persistent button whose behavior delegates to the shared action owner. */
function createChromeButton(
  documentRef: Document,
  key: CubeFaceTitlebarActionKey,
): HTMLButtonElement {
  const button = documentRef.createElement('button');
  button.type = 'button';
  button.dataset.cubeAction = key;
  button.hidden = true;
  return button;
}
