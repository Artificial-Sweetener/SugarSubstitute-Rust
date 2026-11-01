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
/** Qualify media commands against exact current output identities and platform transfer capabilities. */
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type {
  OutputCopyFormat,
  OutputMediaProfile,
} from '../../output_saving/domain/OutputMedia.js';
import { canvasMediaItem } from '../domain/CanvasMedia.js';
import type {
  CanvasCommand,
  CanvasOperationScope,
  CanvasViewSnapshot,
} from '../domain/CanvasViewPort.js';

export interface CanvasMediaOptions {
  read(): CanvasViewSnapshot;
  accepts(): boolean;
  profile?(): OutputMediaProfile;
  copy?(
    item: WorkflowOutputItem,
    accepts: () => boolean,
    format: OutputCopyFormat,
    signal: AbortSignal,
  ): Promise<boolean>;
  /** Reveal or download the item's file; non-image items keep their exact file. */
  fileAction?(
    item: WorkflowOutputItem,
    accepts: () => boolean,
    signal: AbortSignal,
  ): Promise<boolean>;
}

/** Keep late media requests from acting on a replacement workflow, item or newer copy intent. */
export class CanvasMediaActions {
  #operation = 0;
  #disposed = false;
  #pending: {
    controller: AbortController;
    current: () => boolean;
  } | null = null;
  /** Bind the live renderer-neutral state and explicit platform action ports. */
  constructor(private readonly options: CanvasMediaOptions) {}

  /** Advertise only platform-supported commands without teaching menus about browser APIs. */
  commands(): readonly CanvasCommand['type'][] {
    return [
      ...(this.options.copy ? ['copy-media' as const] : []),
      ...(this.options.fileAction ? ['file-media' as const] : []),
    ];
  }

  /** Publish immutable shared action metadata without network requests or image-resource ownership. */
  profile(): OutputMediaProfile | null {
    return this.options.profile?.() ?? null;
  }

  /** Execute a pinned file action and revalidate identity before asynchronous clipboard publication. */
  async run(
    command: Extract<CanvasCommand, { type: 'copy-media' | 'file-media' }>,
    scope: CanvasOperationScope,
  ): Promise<boolean> {
    if (this.#disposed || !this.options.accepts() || !scope.current()) return false;
    const item = canvasMediaItem(this.options.read(), command.target);
    if (!item) return false;
    return await this.#perform(
      () => canvasMediaItem(this.options.read(), command.target)?.id === item.id,
      scope,
      async (accepts, signal) =>
        command.type === 'copy-media'
          ? ((await this.options.copy?.(item, accepts, command.format, signal)) ?? false)
          : ((await this.options.fileAction?.(item, accepts, signal)) ?? false),
    );
  }

  /**
   * Apply the file action to the exact selected output of any media kind.
   *
   * Native presentation opens outputs in place of a browser window through this action.
   */
  async open(item: WorkflowOutputItem, scope: CanvasOperationScope): Promise<boolean> {
    const fileAction = this.options.fileAction;
    if (this.#disposed || !fileAction || !this.options.accepts() || !scope.current()) return false;
    const selected = (): boolean => {
      const snapshot = this.options.read();
      return snapshot.selected?.id === item.id || snapshot.secondary?.id === item.id;
    };
    if (!selected()) return false;
    return await this.#perform(selected, scope, (accepts, signal) =>
      fileAction(item, accepts, signal),
    );
  }

  /** Run one operation that supersedes earlier ones while its pinned output remains current. */
  async #perform(
    current: () => boolean,
    scope: CanvasOperationScope,
    run: (accepts: () => boolean, signal: AbortSignal) => Promise<boolean>,
  ): Promise<boolean> {
    this.#pending?.controller.abort();
    const controller = new AbortController();
    this.#pending = { controller, current };
    const operation = ++this.#operation;
    const cancel = (): void => controller.abort();
    scope.signal.addEventListener('abort', cancel, { once: true });
    if (scope.signal.aborted) cancel();
    const accepts = (): boolean =>
      !this.#disposed &&
      !controller.signal.aborted &&
      scope.current() &&
      operation === this.#operation &&
      this.options.accepts() &&
      current();
    try {
      if (!accepts()) return false;
      return await run(accepts, controller.signal);
    } catch (error) {
      if (controller.signal.aborted && error instanceof DOMException && error.name === 'AbortError')
        return false;
      throw error;
    } finally {
      scope.signal.removeEventListener('abort', cancel);
      if (this.#pending?.controller === controller) this.#pending = null;
    }
  }

  /** Cancel transport when a published state retires the exact originating output. */
  reconcile(): void {
    const pending = this.#pending;
    if (pending && (!this.options.accepts() || !pending.current())) pending.controller.abort();
  }

  /** Invalidate pending work before the attachment and its resources retire. */
  dispose(): void {
    this.#disposed = true;
    this.#pending?.controller.abort();
    this.#operation++;
  }
}
