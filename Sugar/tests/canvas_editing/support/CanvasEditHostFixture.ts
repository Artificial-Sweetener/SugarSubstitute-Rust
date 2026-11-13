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
/** Stand in for a canvas editing host: record commands, answer with state and push changes. */
import type { CanvasEditHost } from '../../../integrations/contracts/CanvasEditing.js';
import type {
  CanvasEditCommand,
  CanvasEditResult,
  CanvasEditState,
  CanvasToolId,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

const ALL_TOOLS: readonly CanvasToolId[] = [
  'pan-zoom',
  'move',
  'brush',
  'eraser',
  'select-rectangle',
  'select-ellipse',
  'select-lasso',
  'mask-rectangle',
  'mask-ellipse',
  'mask-lasso',
  'transform',
  'shared-edges',
];

/** Build a complete edit state with an open document, an active mask and every tool available. */
export function editState(patch: Partial<CanvasEditState> = {}): CanvasEditState {
  return {
    revision: 1,
    document: 'document-1',
    activeMask: 'mask-layer-1',
    tool: 'pan-zoom',
    tools: ALL_TOOLS.map((id) => ({ id, available: true })),
    options: {
      brush: { size: 40, hardness: 80, opacity: 100 },
      eraser: { size: 60, hardness: 50, opacity: 70 },
    },
    selection: null,
    history: { undo: false, redo: false },
    session: null,
    mode: 'none',
    ...patch,
  };
}

/** Wait for queued command replies and the resulting renders. */
export async function settle(): Promise<void> {
  for (let turn = 0; turn < 6; turn++) await Promise.resolve();
}

/**
 * Answer every command with the current state; `reply` overrides the answer per command.
 * `hold` keeps replies pending until `release` so tests can observe queueing.
 */
export class CanvasEditHostFixture implements CanvasEditHost {
  readonly canvasId = 'native-document';
  readonly commands: CanvasEditCommand[] = [];
  state: CanvasEditState;
  reply: ((command: CanvasEditCommand) => CanvasEditResult) | null = null;
  hold = false;
  readonly #held: (() => void)[] = [];
  readonly #listeners = new Set<(state: CanvasEditState) => void>();
  readonly changes = {
    subscribe: (listener: (state: CanvasEditState) => void) => {
      this.#listeners.add(listener);
      return {
        ready: Promise.resolve('observing' as const),
        stop: () => {
          this.#listeners.delete(listener);
        },
      };
    },
  };

  constructor(state: CanvasEditState = editState()) {
    this.state = state;
  }

  /** Report how many listeners currently observe pushed states. */
  get observers(): number {
    return this.#listeners.size;
  }

  execute(command: CanvasEditCommand): Promise<CanvasEditResult> {
    this.commands.push(command);
    const answer = (): CanvasEditResult =>
      this.reply?.(command) ?? { kind: 'state', state: this.state };
    if (!this.hold) return Promise.resolve(answer());
    return new Promise((resolve) => this.#held.push(() => resolve(answer())));
  }

  /** Answer the oldest held command. */
  release(): void {
    this.#held.shift()?.();
  }

  /** Push the next revision with the given changes, as the host does on every state change. */
  push(patch: Partial<CanvasEditState>): CanvasEditState {
    this.state = { ...this.state, ...patch, revision: this.state.revision + 1 };
    for (const listener of [...this.#listeners]) listener(this.state);
    return this.state;
  }

  /** Commands other than the initial state read. */
  sent(): CanvasEditCommand[] {
    return this.commands.filter((command) => command.type !== 'state');
  }
}
