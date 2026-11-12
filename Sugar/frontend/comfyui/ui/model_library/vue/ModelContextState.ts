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
/** Coordinate shared context actions and update dialogs within a native model field's lifetime. */
import { h, onBeforeUnmount, ref, shallowRef } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import type { ModelPickerActions, ModelPickerFeedback } from '../ModelPickerActions.js';
import { canBrowseModelVersions, modelContextEntries } from '../ModelContextMenu.js';
import type { ModelContextEntry } from '../ModelContextMenu.js';
import type { ModelUpdatePort } from '../ModelUpdates.js';
import type { ModelPickerSession } from '../ModelPickerSession.js';
import { ModelDetectorDialog } from '../ModelDetectorDialog.js';
import type { ModelDetectorPort } from '../ModelDetectors.js';
import { ModelContextMenuView } from './ModelContextMenuView.js';
import { useModelAction } from './ModelActionState.js';
import type { ModelOutputThumbnails } from '../ModelOutputThumbnails.js';
import { modelOutputThumbnailMenu } from '../ModelOutputThumbnailMenu.js';
import { projectModelContext } from '../ModelContextProjection.js';

export interface ModelContextOptions {
  readonly actions: ModelPickerActions | null;
  readonly updates: ModelUpdatePort | null;
  readonly feedback: ModelPickerFeedback | null;
  readonly detectors: ModelDetectorPort | null;
  readonly outputs: ModelOutputThumbnails | null;
}

/** Reuse one action owner for banners, tiles and update-icon menus without owning model values. */
export function useModelContext(
  options: ModelContextOptions,
  disabled: () => boolean,
  beforeFamily: () => void,
  session: ModelPickerSession,
) {
  const menu = ref<{ item: ModelCatalogItem; x: number; y: number; updateOnly: boolean } | null>(
    null,
  );
  const updates = shallowRef(options.updates?.read().proposals ?? []);
  const unsubscribe = options.updates?.subscribe((snapshot) => {
    updates.value = snapshot.proposals;
  });
  const state = useModelAction(disabled);
  let dialog: ModelDetectorDialog | null = null;
  let active = true;
  /** Match only authoritative catalog identities rather than loosely matching display labels. */
  const offer = (item: ModelCatalogItem) =>
    updates.value.find((proposal) => proposal.modelId === item.id);
  /** Follow the reference's two-action update-icon menu without exposing provider IDs. */
  const entries = (item: ModelCatalogItem, updateOnly = false): readonly ModelContextEntry[] => {
    const proposal = offer(item);
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
            ? !!options.detectors
            : !!options.actions && /^[a-f0-9]{64}$/.test(item.id),
          !!proposal,
          false,
        ).map((entry) =>
          entry.command === 'output-thumbnail' && options.outputs
            ? modelOutputThumbnailMenu(options.outputs.choices(), options.outputs.activeChoice())
            : entry,
        );
  };
  /** Open a conditional menu at the pointer without committing any native widget callback. */
  const show = (item: ModelCatalogItem, event: MouseEvent, updateOnly = false): void => {
    event.preventDefault();
    event.stopPropagation();
    if (!entries(item, updateOnly).length) return;
    menu.value = { item, x: event.clientX, y: event.clientY, updateOnly };
  };
  /** Browse a verified version family independently of update notification state. */
  const family = (item: ModelCatalogItem): void => {
    if (!options.actions || !canBrowseModelVersions(item, !!offer(item))) return;
    beforeFamily();
    menu.value = null;
    dialog?.close();
    void session
      .versions(item, options.actions, (severity, message) => options.feedback?.(severity, message))
      .finally(() => {
        void options.updates?.load().catch((error: unknown) => {
          options.feedback?.(
            'error',
            error instanceof Error ? error.message : 'Model updates are unavailable.',
          );
        });
      });
  };
  /** Execute reference metadata and update decisions through their authoritative owners. */
  const command = (item: ModelCatalogItem, entry: ModelContextEntry): void => {
    if (entry.command === 'detector-thumbnail' && options.detectors) {
      beforeFamily();
      menu.value = null;
      dialog?.close();
      const current = new ModelDetectorDialog(document, item, options.detectors);
      dialog = current;
      void current.open().finally(() => {
        if (dialog === current) dialog = null;
      });
      return;
    }
    if (entry.command === 'versions') {
      family(item);
      return;
    }
    void state
      .run(async (signal) => {
        if (entry.command === 'refresh')
          await options.actions?.refreshMetadata(
            item.id,
            (message) => {
              state.status.value = message;
            },
            signal,
          );
        else if (
          entry.command === 'output-thumbnail' &&
          entry.imageId &&
          options.outputs &&
          options.actions
        ) {
          const image = await options.outputs.image(entry.imageId, signal);
          if (signal.aborted) return;
          await options.actions.replaceThumbnail(item.id, image, signal);
          if (!signal.aborted) state.status.value = 'Model thumbnail updated.';
        } else if (entry.command === 'dismiss') await options.updates?.dismiss(item.id);
        else if (entry.command === 'disable-updates') await options.updates?.disableModel(item.id);
      })
      .then(() => {
        if (!active) return;
        if (state.error.value) options.feedback?.('error', state.error.value);
        else if (state.status.value) options.feedback?.('info', state.status.value);
      });
  };
  onBeforeUnmount(() => {
    active = false;
    menu.value = null;
    unsubscribe?.();
    dialog?.close();
  });
  /** Render the transient menu with a captured target that survives its close event. */
  const render = () => {
    const current = menu.value;
    return current
      ? h(ModelContextMenuView, {
          entries: entries(current.item, current.updateOnly),
          x: current.x,
          y: current.y,
          onClose: () => {
            menu.value = null;
          },
          onAction: (entry: ModelContextEntry) => command(current.item, entry),
        })
      : null;
  };
  return {
    menu,
    show,
    family,
    offer,
    render,
    proposals: updates,
    /** Capture the selected model without creating another metadata or command owner. */
    project: (item: ModelCatalogItem) =>
      projectModelContext(entries(item), (entry) => command(item, entry)),
  };
}
