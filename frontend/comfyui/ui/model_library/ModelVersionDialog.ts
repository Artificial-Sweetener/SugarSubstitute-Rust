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
/** Coordinate explicit version selection through a live picker receiver and shared download jobs. */
import type { ModelSelectionTarget } from './ModelTransfer.js';
import { mountModelVersionView } from './vue/ModelVersionVueRuntime.js';
import { ModalShell } from '../dialogs/ModalShell.js';
import { chooseModelThumbnail } from './ModelCatalog.js';
import type { ModelCatalogItem } from './ModelCatalog.js';
import { CivitaiKeyDialog } from './CivitaiKeyDialog.js';
import type { ModelPickerActions, ModelPickerFeedback } from './ModelPickerActions.js';
import type { ModelRelease } from './ModelReleases.js';
import { modelProviderPage } from './ModelContextMenu.js';
import { createModelProviderIcon } from './vue/ModelProviderIcon.js';
import { modelActionIcon } from './ModelActionIcon.js';

/** Own version browsing and credential preflight while handing accepted acquisitions to the live field. */
export class ModelVersionDialog {
  /** Report whether a visible family currently owns this document's modal interaction. */
  static isOpen(document: Document): boolean {
    return document.querySelector('.sc-model-version-overlay.is-visible') !== null;
  }

  readonly #shell: ModalShell;
  readonly #document: Document;
  readonly #item: ModelCatalogItem;
  readonly #actions: ModelPickerActions;
  readonly #target: ModelSelectionTarget;
  readonly #feedback: ModelPickerFeedback;
  #keyDialog: CivitaiKeyDialog | null = null;
  #controller: AbortController | null = null;
  #open = false;
  /** Bind the established modal shell and validated acquisition boundary. */
  constructor(
    document: Document,
    item: ModelCatalogItem,
    actions: ModelPickerActions,
    target: ModelSelectionTarget,
    feedback: ModelPickerFeedback,
  ) {
    this.#document = document;
    this.#item = item;
    this.#actions = actions;
    this.#target = target;
    this.#feedback = feedback;
    this.#shell = new ModalShell({
      adapter: { getDocument: () => document, getWindow: () => document.defaultView },
      variantClassName: 'sc-model-version-overlay',
      dialogClassName: 'sc-model-version-modal',
    });
  }
  /** Load compatible versions while deferring every workflow change until deliberate acceptance. */
  async open(): Promise<void> {
    if (this.#open) return;
    this.#open = true;
    let versions: readonly ModelRelease[] = [];
    let selected: ModelRelease | null = null;
    let busy = true;
    const body = this.#document.createElement('div');
    const view = mountModelVersionView(
      body,
      this.#item,
      { versions, selected: '', busy },
      (version) => {
        selected = selected?.versionId === version.versionId ? null : version;
        this.#shell.setConfirmEnabled(selected !== null);
        publish();
      },
    );
    /** Publish state through the isolated Vue renderer after each dialog-owned transition. */
    const publish = (): void => {
      view.update({ versions, selected: selected?.versionId ?? '', busy });
      const title = this.#title(versions);
      if (this.#shell.elements.titleEl) this.#shell.elements.titleEl.textContent = title;
      this.#shell.elements.dialog?.setAttribute('aria-label', title);
      const button = this.#shell.elements.confirmButton;
      if (button) {
        button.textContent = selected?.installedFileName ? 'Use version' : 'Download and use';
        if (!selected?.installedFileName)
          button.prepend(modelActionIcon(this.#document, 'download'));
        this.#shell.setConfirmEnabled(
          !busy &&
            selected !== null &&
            (!!selected.installedFileName || selected.files[0]?.access !== 'unknown'),
        );
      }
    };
    const footer = this.#document.createElement('div');
    footer.className = 'sc-model-family-footer-meta';
    /** Keep provider navigation available when retained identity supplies a missing catalog association. */
    const linkProvider = (url: string): void => {
      const page = modelProviderPage(url);
      if (!page || footer.querySelector('a')) return;
      const link = this.#document.createElement('a');
      link.href = page;
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
      link.textContent = 'View on CivitAI';
      link.prepend(createModelProviderIcon(this.#document, 'civitai'));
      footer.prepend(link);
    };
    linkProvider(this.#item.providerUrl);
    /** Accept an installed version or acquire its verified artifact before committing the host choice. */
    const download = async (): Promise<void> => {
      const version = selected;
      const file = version?.files[0];
      if (!version || busy || (!version.installedFileName && !file)) return;
      if (!version.installedFileName && file?.access === 'unknown') return;
      if (!version.installedFileName && file?.access === 'api_key_required') {
        let keyConfigured = false;
        const controller = new AbortController();
        this.#controller = controller;
        busy = true;
        publish();
        try {
          keyConfigured = (await this.#actions.credentials.status(controller.signal)).configured;
          if (!keyConfigured && this.#open) {
            const prompt = new CivitaiKeyDialog(this.#document, this.#actions.credentials);
            this.#keyDialog = prompt;
            const parent = this.#shell.elements.dialog;
            if (parent) parent.inert = true;
            try {
              keyConfigured = await prompt.open();
            } finally {
              if (parent) parent.inert = false;
              this.#keyDialog = null;
            }
            publish();
          }
        } catch (error: unknown) {
          if (!controller.signal.aborted && this.#open) this.#failure(error);
          return;
        } finally {
          if (this.#controller === controller) this.#controller = null;
          busy = false;
          publish();
          if (this.#open) this.#shell.elements.confirmButton?.focus();
        }
        if (!keyConfigured || !this.#open) return;
      }
      try {
        if (version.installedFileName) {
          this.#target.select(version.installedFileName);
        } else {
          void this.#target
            .useDownload(
              (transfer, signal) =>
                this.#actions.acquire(
                  this.#item.id,
                  version.versionId,
                  file?.fileId ?? '',
                  () => undefined,
                  signal,
                  transfer,
                ),
              {
                name: this.#item.title,
                previewUrl:
                  version.previewUrl ??
                  chooseModelThumbnail(this.#item, 'banner', 768)?.url ??
                  null,
              },
            )
            .catch((error: unknown) => this.#failure(error));
        }
        this.#shell.close(version.versionId);
      } catch (error: unknown) {
        this.#failure(error);
      }
    };
    const closed = this.#shell.open({
      title: this.#title(versions),
      body,
      footerMeta: footer,
      confirmLabel: 'Use version',
      cancelLabel: 'Close',
      allowOverlayClose: false,
      onConfirm: () => {
        void download();
      },
    });
    this.#shell.elements.dialog?.setAttribute('role', 'dialog');

    this.#shell.elements.dialog?.setAttribute('aria-label', this.#title(versions));
    this.#shell.elements.dialog?.setAttribute('aria-modal', 'true');
    this.#shell.setConfirmEnabled(false);
    const controller = new AbortController();
    this.#controller = controller;
    void this.#actions
      .releases(this.#item.id, controller.signal)
      .then((family) => {
        if (controller.signal.aborted || !this.#open) return;
        const current = this.#target.current();
        versions = family
          .map((version) => {
            const inUse = version.installedFileName
              ? version.installedFileName.replaceAll('\\', '/') === current.replaceAll('\\', '/')
              : (version.inUse ?? version.versionId === this.#item.providerVersionId) &&
                current === this.#item.value;
            return {
              ...version,
              inUse,
              onDisk: version.onDisk || inUse,
              installedFileName: version.installedFileName ?? (inUse ? current : null),
            };
          })
          .reverse();
        selected = versions.find((version) => version.inUse) ?? null;
        this.#shell.setConfirmEnabled(selected !== null);
        linkProvider(family.find((version) => version.inUse)?.pageUrl ?? family[0]?.pageUrl ?? '');
        if (!family.length) this.#feedback('info', 'No compatible model versions were found.');
      })
      .catch((error: unknown) => {
        if (!controller.signal.aborted && this.#open) {
          this.#failure(error);
        }
      })
      .finally(() => {
        if (this.#controller === controller) this.#controller = null;
        busy = false;
        publish();
      });
    try {
      await closed;
    } finally {
      this.#open = false;
      this.#keyDialog?.close();
      this.#keyDialog = null;
      this.#controller?.abort();
      this.#controller = null;
      view.dispose();
      if (this.#shell.elements.overlay) {
        this.#shell.elements.overlay.remove();
      }
    }
  }
  /** Dismiss browsing and cancel unaccepted lookup or credential work. */
  close(): void {
    this.#shell.close(null);
  }
  /** Present prepared architecture and family labels without inferring missing model facts. */
  #title(versions: readonly ModelRelease[]): string {
    const architecture = this.#item.traits?.architecture || this.#item.architecture?.base || '';
    const family =
      this.#item.traits?.familyName ||
      versions.find((version) => version.inUse)?.baseModel ||
      this.#item.baseModel;
    const baseFamily =
      !!architecture &&
      [this.#item.traits?.family ?? '', family].some(
        (label) => label.trim().toLowerCase() === architecture.trim().toLowerCase(),
      );
    const labels = [architecture, baseFamily ? 'Base Model' : family].filter(Boolean).join(' \\ ');
    return `Versions of ${this.#item.title}${labels ? ` - ${labels}` : ''}`;
  }
  /** Report actionable toast feedback while retaining diagnostic context. */
  #failure(error: unknown): void {
    this.#feedback(
      'error',
      error instanceof Error ? error.message : 'Model versions are unavailable.',
    );
    console.error('SugarCubes model version action failed.', error);
  }
}
