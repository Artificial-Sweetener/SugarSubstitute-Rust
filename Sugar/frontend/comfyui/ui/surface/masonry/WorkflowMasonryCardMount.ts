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
/** Reuse a mounted Cube card or own a temporary native projection for one masonry cell. */
import type { WorkflowMasonryRebind } from './WorkflowMasonryRemountQueue.js';
import type { ComfyNode } from '../../types/graph.js';
import type { ProjectedNodeCard } from '../NodeCardProjection.js';
import { NativeNodeCardHost } from '../NativeNodeCardHost.js';
import type { NativeNodeCardRenderer } from '../NativeNodeCardRenderer.js';
import {
  prepareNativeCardPresentation,
  type NativeCardPresentationTransfer,
} from '../NativeCardPresentationOwner.js';

export interface WorkflowMasonryCardMountOptions {
  cell: HTMLElement;
  card: ProjectedNodeCard;
  renderer: NativeNodeCardRenderer;
  source: HTMLElement | null;
  requestRemount(rebind: WorkflowMasonryRebind): void;
  onActivationChange(node: ComfyNode, enabled: boolean): void;
}

/** Release only presentation resources owned here, keeping source native widgets alive. */
export class WorkflowMasonryCardMount {
  readonly #options: WorkflowMasonryCardMountOptions;
  #card: ProjectedNodeCard;
  #transfer: NativeCardPresentationTransfer | null;
  #host: NativeNodeCardHost | null = null;
  #disposed = false;
  #mounted = false;

  /** Prepare source reads separately from the later DOM relocation or native mount. */
  constructor(options: WorkflowMasonryCardMountOptions) {
    this.#options = options;
    this.#card = options.card;
    this.#transfer = options.source
      ? prepareNativeCardPresentation(options.source, this.#onInvalidated)
      : null;
  }

  /** Budget frame yields for newly rendered controls rather than cheap native cell transfers. */
  get requiresNativeMount(): boolean {
    return this.#transfer === null;
  }

  /** Attach one native control tree, preferring its existing Cube-face presentation. */
  mount(): void {
    if (this.#disposed || this.#mounted) return;
    this.#mounted = true;
    if (this.#transfer) this.#transfer.move(this.#options.cell);
    else {
      this.#host = new NativeNodeCardHost(this.#options.renderer);
      this.#host.mountCard(this.#options.cell, this.#card, this.#options.onActivationChange, {
        allowCollapse: this.#card.prompt !== true,
      });
    }
  }

  /** Refresh shared activation data without rebuilding controls or replacing a borrowed renderer. */
  refresh(card: ProjectedNodeCard): void {
    this.#card = card;
    this.#host?.refresh([card]);
    const cell = this.#transfer?.element ?? this.#options.cell;
    cell.dataset.cubeColumnSpan = String(card.columnSpan ?? 1);
  }

  /** Return a borrowed native cell or release a temporary native projection. */
  dispose(): void {
    this.#disposed = true;
    this.#transfer?.restore();
    this.#transfer = null;
    this.#host?.dispose();
    this.#host = null;
  }

  /** Reacquire only this card after its source owner replaces controls during a policy change. */
  readonly #onInvalidated = (): void => {
    this.#options.requestRemount((source) => {
      if (this.#disposed) return false;
      this.#transfer = source ? prepareNativeCardPresentation(source, this.#onInvalidated) : null;
      this.#host?.dispose();
      this.#host = null;
      this.#mounted = false;
      this.mount();
      return true;
    });
  };
}
