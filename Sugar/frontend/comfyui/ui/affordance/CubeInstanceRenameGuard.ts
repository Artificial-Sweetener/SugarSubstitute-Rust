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
/** Preserve Cube definition identity while Comfy edits a graph-local instance title. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { CubeNodeCatalog } from '../cube/node/CubeNodeCatalog.js';
import type { CubeEditorContextResolver } from '../surface/CubeEditorContextResolver.js';

interface RenameCanvas {
  selectedItems: Set<unknown>;
}

interface RenameSession {
  node: CubeNode;
  definitionName: string;
  instanceTitle: string | undefined;
  restoreTimer: number | null;
}

/** Guard the stable native title editor rather than changing Subgraph structural identity. */
export class CubeInstanceRenameGuard {
  readonly #document: Document;
  readonly #canvas: RenameCanvas;
  readonly #nodes: CubeNodeCatalog;
  readonly #contexts: CubeEditorContextResolver;
  readonly #sessions = new Map<HTMLInputElement, RenameSession>();
  readonly #handleFocusIn: (event: FocusEvent) => void;
  readonly #handleFocusOut: (event: FocusEvent) => void;
  readonly #handleKeydown: (event: KeyboardEvent) => void;

  /** Bind both renderer title editors through Comfy's shared stable test id. */
  constructor(options: {
    document: Document;
    canvas: RenameCanvas;
    nodes: CubeNodeCatalog;
    contexts: CubeEditorContextResolver;
  }) {
    this.#document = options.document;
    this.#canvas = options.canvas;
    this.#nodes = options.nodes;
    this.#contexts = options.contexts;
    this.#handleFocusIn = (event) => this.#onFocusIn(event);
    this.#handleFocusOut = (event) => this.#onFocusOut(event);
    this.#handleKeydown = (event) => this.#onKeydown(event);
  }

  /** Observe title-edit commits before renderer-specific handlers synchronize names. */
  install(): void {
    this.#document.addEventListener('focusin', this.#handleFocusIn, true);
    this.#document.addEventListener('focusout', this.#handleFocusOut, true);
    this.#document.addEventListener('keydown', this.#handleKeydown, true);
  }

  /** Release listeners and preserve any definition involved in an active edit. */
  dispose(): void {
    this.#document.removeEventListener('focusin', this.#handleFocusIn, true);
    this.#document.removeEventListener('focusout', this.#handleFocusOut, true);
    this.#document.removeEventListener('keydown', this.#handleKeydown, true);
    for (const session of this.#sessions.values()) this.#restore(session);
    this.#sessions.clear();
  }

  /** Snapshot shared definition identity when a Cube instance title editor opens. */
  #onFocusIn(event: FocusEvent): void {
    const input = titleInput(event.target);
    if (!input || this.#sessions.has(input)) return;
    const node = this.#resolveNode(input);
    if (!node) return;
    this.#sessions.set(input, {
      node,
      definitionName: node.subgraph.name,
      instanceTitle: node.title,
      restoreTimer: null,
    });
  }

  /** Restore after blur-driven commits finish propagating through Vue. */
  #onFocusOut(event: FocusEvent): void {
    const input = titleInput(event.target);
    const session = input ? this.#sessions.get(input) : undefined;
    if (!input || !session) return;
    this.#scheduleRestore(input, session);
  }

  /** Restore after keyboard commits even when the host removes the input before blur. */
  #onKeydown(event: KeyboardEvent): void {
    if (event.key !== 'Enter') return;
    const input = titleInput(event.target);
    const session = input ? this.#sessions.get(input) : undefined;
    if (!input || !session) return;
    this.#scheduleRestore(input, session);
  }

  /** Restore after Vue and the native subgraph store finish the same commit turn. */
  #scheduleRestore(input: HTMLInputElement, session: RenameSession): void {
    if (session.restoreTimer !== null) return;
    const view = this.#document.defaultView;
    if (!view) {
      queueMicrotask(() => {
        this.#restore(session);
        this.#sessions.delete(input);
      });
      return;
    }
    session.restoreTimer = view.setTimeout(() => {
      this.#restore(session);
      this.#sessions.delete(input);
    }, 0);
  }

  /** Resolve Nodes 2.0 by header id and Nodes 1.0 by the authoritative selection. */
  #resolveNode(input: HTMLInputElement): CubeNode | null {
    const header = input.closest<HTMLElement>('[data-testid^="node-header-"]');
    const headerId = header?.dataset.testid?.slice('node-header-'.length);
    if (headerId) {
      return this.#nodes.list().find((node) => String(node.id) === headerId) ?? null;
    }
    const selection = this.#contexts.resolveSelection(this.#canvas.selectedItems);
    return selection.cubeNodes.length === 1 && selection.ordinarySubgraphNodes.length === 0
      ? (selection.cubeNodes[0] ?? null)
      : null;
  }

  /** Undo only Comfy's definition-name coupling and retain the local title change. */
  #restore(session: RenameSession): void {
    const view = this.#document.defaultView;
    if (view && session.restoreTimer !== null) view.clearTimeout(session.restoreTimer);
    session.restoreTimer = null;
    if (session.node.subgraph.name !== session.definitionName) {
      session.node.subgraph.name = session.definitionName;
    }
    if (session.node.title !== session.instanceTitle) this.#nodes.changed(session.node);
  }
}

/** Narrow event targets to Comfy's shared title input contract. */
function titleInput(value: EventTarget | null): HTMLInputElement | null {
  return value instanceof HTMLInputElement && value.dataset.testid === 'node-title-input'
    ? value
    : null;
}
