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
/** Own reference discovery, explicit credentials, acquisition, and contained recoverable feedback. */

import type { ModelSelectionTarget, ModelTransferObserver } from './ModelTransfer.js';
import { ModalShell } from '../dialogs/ModalShell.js';
import { modelActionIcon } from './ModelActionIcon.js';
import { mountModelDiscoveryView } from './vue/ModelDiscoveryVueRuntime.js';
import type {
  ModelDiscoveryPort,
  CivitaiCredentialPort,
  CivitaiCredentialStatus,
} from './ModelDiscovery.js';
import type { ModelDiscoveryViewState, MountedModelDiscoveryView } from './ModelDiscoveryView.js';
import { modelDiscoveryCss } from './ModelDiscoveryStyles.js';

/** Coordinate one modal while keeping workflow values owned by the supplied live field receiver. */
export class ModelDiscoveryDialog {
  readonly #document: Document;
  readonly #kind: string;
  readonly #targetModel: string;
  readonly #discovery: ModelDiscoveryPort;
  readonly #credentials: CivitaiCredentialPort;
  readonly #installed: (filename: string) => void;
  readonly #shell: ModalShell;
  readonly #target: ModelSelectionTarget | null;
  #view: MountedModelDiscoveryView | null = null;
  #state: ModelDiscoveryViewState;
  #credentialStatus: CivitaiCredentialStatus = {
    configured: false,
    storageAvailable: false,
    storageReason: null,
  };
  #controller: AbortController | null = null;
  #open = false;

  /** Bind shared authorities and the receiver that resolves current legal Comfy values after refresh. */
  constructor(
    document: Document,
    kind: string,
    targetModel: string,
    discovery: ModelDiscoveryPort,
    credentials: CivitaiCredentialPort,
    installed: (filename: string) => void,
    target: ModelSelectionTarget | null = null,
  ) {
    this.#document = document;
    this.#kind = kind;
    this.#targetModel = targetModel;
    this.#discovery = discovery;
    this.#credentials = credentials;
    this.#installed = installed;
    this.#target = target;
    this.#shell = new ModalShell({
      adapter: { getDocument: () => document, getWindow: () => document.defaultView },
      variantClassName: 'sc-model-discovery-overlay',
      dialogClassName: 'sc-model-discovery-modal',
    });
    this.#state = {
      artifactKind: kind,
      plan: null,
      loading: true,
      downloading: false,
      selected: '',
      providers: {},
      status: 'Finding compatible models…',
      credential: false,
      credentialBusy: false,
      credentialError: '',
    };
  }

  /** Present loading immediately and release every view-owned resource on close. */
  async open(): Promise<void> {
    if (this.#open) return;
    this.#open = true;
    const body = this.#document.createElement('div');
    body.className = 'sc-model-discovery-body';
    const style = this.#document.createElement('style');
    style.textContent = modelDiscoveryCss;
    this.#view = mountModelDiscoveryView(body, this.#state, {
      select: (identity) => this.#update({ selected: identity }),
      provider: (identity, providerId) =>
        this.#update({ providers: { ...this.#state.providers, [identity]: providerId } }),
      cancelCredential: () => this.#update({ credential: false, credentialError: '' }),
      publicOnly: () => {
        void this.#load(true);
      },
      saveCredential: (key) => {
        void this.#saveCredential(key);
      },
    });
    const footer = this.#document.createElement('div');
    footer.className = 'sc-model-discovery-footer';
    const explore = this.#document.createElement('a');
    explore.className = 'p-button p-button-secondary';
    explore.textContent = 'Explore more';
    explore.target = '_blank';
    explore.rel = 'noopener noreferrer';
    explore.prepend(modelActionIcon(this.#document, 'globe'));
    explore.hidden = true;
    const all = this.#document.createElement('button');
    all.type = 'button';
    all.className = 'p-button p-button-secondary';
    all.textContent = 'Show all models';
    all.hidden = true;
    all.addEventListener('click', () => {
      void this.#load(false);
    });
    footer.append(explore, all);
    const closed = this.#shell.open({
      title:
        this.#kind === 'upscale_models'
          ? 'Download an upscaler model?'
          : 'Download an image model?',
      body,
      footerMeta: footer,
      cancelLabel: 'Not now',
      confirmLabel: 'Download and use',
      allowOverlayClose: false,
      onConfirm: () => {
        void this.#download();
      },
    });
    const dialog = this.#shell.elements.dialog;
    dialog?.setAttribute('role', 'dialog');
    dialog?.setAttribute('aria-modal', 'true');
    dialog?.setAttribute(
      'aria-label',
      this.#kind === 'upscale_models' ? 'Download an upscaler model?' : 'Download an image model?',
    );
    dialog?.append(style);
    this.#publish();
    void this.#load(false);
    try {
      await closed;
    } finally {
      this.#open = false;
      this.#controller?.abort();
      this.#controller = null;
      const plan = this.#state.plan;
      if (plan) this.#release(plan.planId);
      this.#view.dispose();
      this.#view = null;
      if (this.#shell.elements.overlay) {
        this.#shell.elements.overlay.remove();
      }
    }
  }

  /** Cancel owned work when its originating node or supported renderer is removed. */
  close(): void {
    this.#shell.close(null);
  }

  /** Replace provider plans only through fresh current-user or public-only discovery. */
  async #load(publicOnly: boolean): Promise<void> {
    if (!this.#open || this.#state.downloading) return;
    this.#controller?.abort();
    const previous = this.#state.plan;
    if (previous) this.#release(previous.planId);
    const controller = new AbortController();
    this.#controller = controller;
    this.#update({
      plan: null,
      selected: '',
      providers: {},
      loading: true,
      credential: false,
      credentialError: '',
      status: 'Finding compatible models…',
    });
    try {
      const [plan, credentials] = await Promise.all([
        this.#discovery.discover(
          this.#kind,
          this.#targetModel,
          publicOnly,
          (status) => {
            if (!controller.signal.aborted) this.#update({ status });
          },
          controller.signal,
        ),
        this.#credentials.status(controller.signal).catch((error: unknown) => {
          if (!controller.signal.aborted)
            console.error('SugarCubes credential status lookup failed.', error);
          return { configured: false, storageAvailable: false, storageReason: null };
        }),
      ]);
      if (controller.signal.aborted || !this.#open) {
        this.#release(plan.planId);
        return;
      }
      if (plan.artifactKind !== this.#kind) {
        this.#release(plan.planId);
        throw new Error('Model discovery returned a different artifact role.');
      }
      this.#credentialStatus = credentials;
      this.#update({
        plan,
        loading: false,
        status: plan.suggestions.length
          ? ''
          : publicOnly
            ? 'No models that can be downloaded without a key are available right now.'
            : 'No compatible models are available right now.',
      });
    } catch (error: unknown) {
      if (!controller.signal.aborted && this.#open) this.#failure(error);
    } finally {
      if (this.#controller === controller) this.#controller = null;
    }
  }

  /** Prompt only for the selected protected offer, then acquire that exact reviewed source. */
  async #download(): Promise<void> {
    const plan = this.#state.plan;
    const item = plan?.suggestions.find((value) => value.identity === this.#state.selected);
    const offer =
      item?.offers.find((value) => value.providerId === this.#state.providers[item.identity]) ??
      item?.offers[0];
    if (
      !plan ||
      !item ||
      !offer ||
      this.#state.downloading ||
      this.#state.loading ||
      this.#state.credential
    )
      return;
    if (offer.access === 'api_key_required' && !this.#credentialStatus.configured) {
      this.#update({
        credential: true,
        credentialError: this.#credentialStatus.storageAvailable
          ? ''
          : (this.#credentialStatus.storageReason ??
            'Secure credential storage is unavailable. You can choose a public model.'),
      });
      return;
    }
    const controller = new AbortController();
    this.#controller = controller;
    this.#update({ downloading: true, status: 'Downloading model…' });
    try {
      const operation = (transfer?: ModelTransferObserver, signal = controller.signal) =>
        this.#discovery.acquire(
          plan.planId,
          item.identity,
          offer.providerId,
          (status) => {
            if (!controller.signal.aborted) this.#update({ status });
          },
          signal,
          transfer,
        );
      const filename = this.#target
        ? await this.#target.download(
            operation,
            { name: item.modelName, previewUrl: offer.previewUrl },
            controller.signal,
          )
        : await operation();
      if (controller.signal.aborted || !this.#open) return;
      this.#installed(filename);
      this.#shell.close(filename);
    } catch (error: unknown) {
      if (!controller.signal.aborted && this.#open) this.#failure(error);
    } finally {
      if (this.#controller === controller) this.#controller = null;
      if (this.#open) this.#update({ downloading: false });
    }
  }

  /** Save entered material through the secure owner and resume the same protected selection. */
  async #saveCredential(key: string): Promise<void> {
    if (!this.#open || this.#state.credentialBusy) return;
    const controller = new AbortController();
    this.#controller = controller;
    this.#update({ credentialBusy: true, credentialError: '' });
    try {
      const status = await this.#credentials.save(key, controller.signal);
      if (controller.signal.aborted || !this.#open) return;
      if (!status.configured) throw new Error('The CivitAI credential could not be saved.');
      this.#credentialStatus = status;
      this.#update({ credential: false, credentialBusy: false });
      await this.#download();
    } catch (error: unknown) {
      if (!controller.signal.aborted && this.#open) {
        this.#update({
          credentialBusy: false,
          credentialError:
            error instanceof Error ? error.message : 'The CivitAI credential could not be saved.',
        });
        console.error('SugarCubes secure credential command failed.', error);
      }
    } finally {
      if (this.#controller === controller) this.#controller = null;
    }
  }

  /** Apply one coordinator-owned state transition and update footer controls atomically. */
  #update(change: Partial<ModelDiscoveryViewState>): void {
    this.#state = { ...this.#state, ...change };
    this.#publish();
  }

  /** Derive footer availability from the exact selection without auto-selecting suggestions. */
  #publish(): void {
    this.#view?.update(this.#state);
    const { confirmButton, cancelButton, footerMeta, footer, titleEl } = this.#shell.elements;
    const selected = this.#state.plan?.suggestions.find(
      (item) => item.identity === this.#state.selected,
    );
    const offer =
      selected?.offers.find(
        (item) => item.providerId === this.#state.providers[selected.identity],
      ) ?? selected?.offers[0];
    const needsKey = offer?.access === 'api_key_required' && !this.#credentialStatus.configured;
    if (confirmButton) {
      confirmButton.textContent = needsKey ? 'Add CivitAI key' : 'Download and use';
      confirmButton.prepend(modelActionIcon(this.#document, needsKey ? 'key' : 'download'));
      confirmButton.disabled =
        !selected || this.#state.loading || this.#state.downloading || this.#state.credential;
    }
    if (cancelButton) cancelButton.disabled = this.#state.downloading;
    const explore = footerMeta?.querySelector<HTMLAnchorElement>('a');
    const browse = this.#state.plan?.browseUrls[0];
    if (explore) {
      explore.hidden = !browse;
      if (browse) explore.href = browse.url;
    }
    const all = footerMeta?.querySelector<HTMLButtonElement>('button');
    if (all) {
      all.hidden = this.#state.plan?.publicOnly !== true;
      all.disabled = this.#state.downloading;
    }
    footer?.toggleAttribute('inert', this.#state.credential);
    titleEl?.closest('.p-dialog-header')?.toggleAttribute('inert', this.#state.credential);
  }

  /** Keep provider failures inside the existing suggestion surface with diagnostic context. */
  #failure(error: unknown): void {
    this.#update({
      loading: false,
      status: error instanceof Error ? error.message : 'Model discovery is unavailable.',
    });
    console.error('SugarCubes model discovery failed.', error);
  }

  /** Release reviewed evidence without turning cleanup failures into blocking user dialogs. */
  #release(planId: string): void {
    void this.#discovery
      .release(planId)
      .catch((error: unknown) =>
        console.error('SugarCubes discovery lease cleanup failed.', error),
      );
  }
}
