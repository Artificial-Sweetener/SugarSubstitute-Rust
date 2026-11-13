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
import type { CanvasCatalogEntry } from '../domain/CanvasCatalog.js';
import {
  canvasMediaImages,
  canvasMediaItem,
  canvasMediaFile,
  type CanvasOutputActions,
} from '../domain/CanvasMedia.js';
import type {
  CanvasCommand,
  CanvasOperationScope,
  CanvasViewSnapshot,
} from '../domain/CanvasViewPort.js';

export interface CanvasMediaOptions {
  read(): CanvasViewSnapshot;
  accepts(): boolean;
  /** Every source of the current result with its final items, in catalog order. */
  entries?(): readonly CanvasCatalogEntry[];
  profile?(item?: WorkflowOutputItem): OutputMediaProfile;
  capabilities?(
    item: WorkflowOutputItem,
  ): Pick<CanvasOutputActions, 'copy' | 'file' | 'photoshop' | 'drag'>;
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
  /** Open still images as named layers of one new Photoshop document. */
  photoshop?(
    document: string,
    layers: readonly CanvasPhotoshopLayer[],
    accepts: () => boolean,
    signal: AbortSignal,
  ): Promise<boolean>;
}

/** One still image and the layer name it takes in Photoshop. */
export interface CanvasPhotoshopLayer {
  readonly item: WorkflowOutputItem;
  readonly name: string;
}

type MediaCommand = Extract<
  CanvasCommand,
  { type: 'copy-media' | 'file-media' | 'photoshop-media' | 'photoshop-all-media' }
>;

/** Keep late media requests from acting on a replacement workflow, item or newer copy intent. */
export class CanvasMediaActions {
  #operation = 0;
  #disposed = false;
  #actionCache: {
    readonly entries: readonly CanvasCatalogEntry[];
    readonly profile: OutputMediaProfile | null;
    readonly actions: readonly CanvasOutputActions[];
  } | null = null;
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
      ...(this.options.photoshop ? (['photoshop-media', 'photoshop-all-media'] as const) : []),
    ];
  }

  /** Publish immutable shared action metadata without network requests or image-resource ownership. */
  profile(): OutputMediaProfile | null {
    return this.options.profile?.() ?? null;
  }

  /** Cache per-final capability facts at catalog/profile boundaries rather than camera or playback updates. */
  itemActions(): readonly CanvasOutputActions[] {
    const entries = this.options.entries?.() ?? [];
    const profile = this.profile();
    if (this.#actionCache?.entries === entries && this.#actionCache.profile === profile)
      return this.#actionCache.actions;
    const actions = Object.freeze(
      entries.flatMap((entry) => entry.items.map((item) => this.#actions(item))),
    );
    this.#actionCache = { entries, profile, actions };
    return actions;
  }

  /** Require every intended image layer and refuse an unresolved final instead of silently dropping it. */
  photoshopAllAvailable(): boolean {
    const entries = this.options.entries?.() ?? [];
    const items = canvasMediaImages(entries);
    return (
      items.length > 0 &&
      !entries.some((entry) => entry.items.some((item) => item.unavailable)) &&
      items.every((item) => this.#actions(item).photoshop)
    );
  }

  /** Execute a pinned file action and revalidate identity before asynchronous clipboard publication. */
  async run(command: MediaCommand, scope: CanvasOperationScope): Promise<boolean> {
    if (this.#disposed || !this.options.accepts() || !scope.current()) return false;
    if (command.type === 'photoshop-all-media') return await this.#photoshopAll(scope);
    const item =
      command.type === 'file-media'
        ? canvasMediaFile(this.options.read(), command.target)
        : canvasMediaItem(this.options.read(), command.target);
    if (!item) return false;
    const actions = this.#actions(item);
    if (
      (command.type === 'copy-media' && !actions.copy) ||
      (command.type === 'file-media' && !actions.file) ||
      (command.type === 'photoshop-media' && !actions.photoshop)
    )
      return false;
    return await this.#perform(
      () => canvasMediaFile(this.options.read(), command.target)?.id === item.id,
      scope,
      async (accepts, signal) => {
        switch (command.type) {
          case 'copy-media':
            return (await this.options.copy?.(item, accepts, command.format, signal)) ?? false;
          case 'file-media':
            return (await this.options.fileAction?.(item, accepts, signal)) ?? false;
          case 'photoshop-media': {
            const layer = { item, name: this.#layerName(item) };
            return (await this.options.photoshop?.(layer.name, [layer], accepts, signal)) ?? false;
          }
        }
      },
    );
  }

  /**
   * Open every still image of the current result as layers of one document, in catalog order.
   *
   * The request stays pinned to that exact image set: a newer result or a retired image cancels it.
   */
  async #photoshopAll(scope: CanvasOperationScope): Promise<boolean> {
    const photoshop = this.options.photoshop;
    const items = canvasMediaImages(this.options.entries?.() ?? []);
    const first = items[0];
    if (!photoshop || !first || !this.photoshopAllAvailable()) return false;
    const identity = JSON.stringify(items.map((item) => item.id));
    const layers = items.map((item) => ({ item, name: this.#layerName(item) }));
    return await this.#perform(
      () =>
        JSON.stringify(canvasMediaImages(this.options.entries?.() ?? []).map((item) => item.id)) ===
        identity,
      scope,
      (accepts, signal) =>
        photoshop(first.file.filename.replace(/\.[^.]*$/, ''), layers, accepts, signal),
    );
  }

  /** Name a layer after its output source and one-based batch position. */
  #layerName(item: WorkflowOutputItem): string {
    const label =
      (this.options.entries?.() ?? []).find((entry) => entry.source.id === item.provenance.sourceId)
        ?.source.label ?? 'Output';
    return `${label} ${String(item.batchIndex + 1)}`;
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
    if (!this.#actions(item).file) return false;
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
    this.#actionCache = null;
  }

  /** Combine platform support with current artifact availability, preserving accurate per-item file actions. */
  #actions(item: WorkflowOutputItem): CanvasOutputActions {
    const profile = this.options.profile?.(item);
    const capability = this.options.capabilities?.(item);
    const available = !item.unavailable && item.mediaKind !== 'unsupported';
    return Object.freeze({
      itemId: item.id,
      copy:
        available &&
        item.mediaKind === 'image' &&
        !!this.options.copy &&
        (capability?.copy ?? true),
      file: available && !!this.options.fileAction && (capability?.file ?? true),
      drag: available && item.mediaKind === 'image' && (capability?.drag ?? true),
      photoshop:
        available &&
        item.mediaKind === 'image' &&
        !!this.options.photoshop &&
        (capability?.photoshop ?? profile?.photoshop ?? true),
      fileAction: profile?.fileAction ?? 'download',
    });
  }
}
