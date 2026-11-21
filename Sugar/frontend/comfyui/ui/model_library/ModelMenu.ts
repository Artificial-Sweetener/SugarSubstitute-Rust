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
/** Own every row about a model and every command those rows run, for every model surface. */
import type { ContextMenuEntry } from '../menus/ContextMenu.js';
import type { ModelCatalogItem } from './ModelCatalog.js';
import type { ModelPickerActions, ModelPickerFeedback } from './ModelPickerActions.js';
import { canBrowseModelVersions, modelContextEntries } from './ModelContextMenu.js';
import type { ModelContextEntry } from './ModelContextMenu.js';
import type { ModelUpdateOffer, ModelUpdatePort } from './ModelUpdates.js';
import type { ModelPickerSession } from './ModelPickerSession.js';
import type { ModelDetectorPort } from './ModelDetectors.js';
import type { ModelOutputThumbnails } from './ModelOutputThumbnails.js';
import { modelOutputThumbnailMenu } from './ModelOutputThumbnailMenu.js';
import { projectModelContext } from './ModelContextProjection.js';

/** The owners behind a model's rows; a surface without one leaves its rows out. */
export interface ModelContextOptions {
  readonly actions: ModelPickerActions | null;
  readonly updates: ModelUpdatePort | null;
  readonly feedback: ModelPickerFeedback | null;
  readonly detectors: ModelDetectorPort | null;
  readonly outputs: ModelOutputThumbnails | null;
  /** Opens the detector thumbnail chooser for one detector; null where it cannot open. */
  readonly detectorDialog: ((item: ModelCatalogItem) => ModelDetectorDialogHandle) | null;
}

/** One open detector thumbnail chooser. */
export interface ModelDetectorDialogHandle {
  open(): Promise<void>;
  close(): void;
}

/**
 * One model menu for the picker field, every wall card, the update badge and the prompt's LoRA
 * banner: the same rows, the same commands, whichever surface opened it.
 */
export class ModelMenu {
  #proposals: readonly ModelUpdateOffer[];
  readonly #unsubscribe: (() => void) | undefined;
  readonly #listeners = new Set<() => void>();
  #controller: AbortController | null = null;
  #dialog: ModelDetectorDialogHandle | null = null;
  #disposed = false;

  /** Bind the owners, the session that applies a chosen version, and the caller's hooks. */
  constructor(
    private readonly options: ModelContextOptions,
    private readonly session: ModelPickerSession | null,
    private readonly disabled: () => boolean = () => false,
    private readonly beforeFamily: () => void = () => undefined,
  ) {
    this.#proposals = options.updates?.read().proposals ?? [];
    this.#unsubscribe = options.updates?.subscribe((snapshot) => {
      this.#proposals = snapshot.proposals;
      for (const listener of this.#listeners) listener();
    });
  }

  /** The current update offers, in catalog order. */
  get proposals(): readonly ModelUpdateOffer[] {
    return this.#proposals;
  }

  /** Observe changed update offers. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Match only authoritative catalog identities rather than loosely matching display labels. */
  offer(item: ModelCatalogItem): ModelUpdateOffer | undefined {
    return this.#proposals.find((proposal) => proposal.modelId === item.id);
  }

  /** The rows about `item`; the update badge's own menu holds only its two update decisions. */
  entries(item: ModelCatalogItem, updateOnly = false): readonly ModelContextEntry[] {
    const proposal = this.offer(item);
    const { actions, detectors, outputs } = this.options;
    return updateOnly && proposal
      ? [
          { label: 'Dismiss', enabled: true, command: 'dismiss', url: null },
          {
            label: `Don't check for updates for ${proposal.modelName}`,
            enabled: true,
            command: 'disable-updates',
            url: null,
          },
        ]
      : modelContextEntries(
          item,
          item.kind === 'ultralytics'
            ? !!detectors && !!this.options.detectorDialog
            : !!actions && /^[a-f0-9]{64}$/.test(item.id),
          !!proposal,
          false,
        ).map((entry) =>
          entry.command === 'output-thumbnail' && outputs
            ? modelOutputThumbnailMenu(outputs.choices(), outputs.activeChoice())
            : entry,
        );
  }

  /** The rows about `item` as shared menu entries, for menus that are not the picker's own. */
  project(item: ModelCatalogItem, updateOnly = false): readonly ContextMenuEntry[] {
    return projectModelContext(this.entries(item, updateOnly), (entry) =>
      this.command(item, entry),
    );
  }

  /** Browse a verified version family independently of update notification state. */
  family(item: ModelCatalogItem): void {
    const { actions, feedback, updates } = this.options;
    if (!actions || !this.session || !canBrowseModelVersions(item, !!this.offer(item))) return;
    this.beforeFamily();
    this.#dialog?.close();
    void this.session
      .versions(item, actions, (severity, message) => feedback?.(severity, message))
      .finally(() => {
        void updates?.load().catch((error: unknown) => {
          feedback?.(
            'error',
            error instanceof Error ? error.message : 'Model updates are unavailable.',
          );
        });
      });
  }

  /** Execute one row through its authoritative owner and report how it ended. */
  command(item: ModelCatalogItem, entry: ModelContextEntry): void {
    const { actions, outputs, updates, feedback, detectorDialog } = this.options;
    if (entry.command === 'detector-thumbnail' && detectorDialog) {
      this.beforeFamily();
      this.#dialog?.close();
      const current = detectorDialog(item);
      this.#dialog = current;
      void current.open().finally(() => {
        if (this.#dialog === current) this.#dialog = null;
      });
      return;
    }
    if (entry.command === 'versions') {
      this.family(item);
      return;
    }
    let status = '';
    void this.#run(async (signal) => {
      if (entry.command === 'refresh')
        await actions?.refreshMetadata(
          item.id,
          (message) => {
            status = message;
          },
          signal,
        );
      else if (entry.command === 'output-thumbnail' && entry.imageId && outputs && actions) {
        const image = await outputs.image(entry.imageId, signal);
        if (signal.aborted) return;
        await actions.replaceThumbnail(item.id, image, signal);
        if (!signal.aborted) status = 'Model thumbnail updated.';
      } else if (entry.command === 'dismiss') await updates?.dismiss(item.id);
      else if (entry.command === 'disable-updates') await updates?.disableModel(item.id);
    }).then((error) => {
      if (this.#disposed) return;
      if (error) feedback?.('error', error);
      else if (status) feedback?.('info', status);
    });
  }

  /** Release the update subscription, a running command and an open detector dialog. */
  dispose(): void {
    this.#disposed = true;
    this.#unsubscribe?.();
    this.#listeners.clear();
    this.#controller?.abort();
    this.#dialog?.close();
  }

  /** Run one command at a time; resolve with its failure message, or null. */
  async #run(operation: (signal: AbortSignal) => Promise<void>): Promise<string | null> {
    if (this.#controller || this.disabled()) return null;
    const current = new AbortController();
    this.#controller = current;
    try {
      await operation(current.signal);
      return null;
    } catch (failure: unknown) {
      if (current.signal.aborted) return null;
      console.error('SugarCubes model action failed.', failure);
      return failure instanceof Error ? failure.message : 'The model action failed.';
    } finally {
      this.#controller = null;
    }
  }
}
