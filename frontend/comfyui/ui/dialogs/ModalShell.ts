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
/**
 * Own the shared SugarCubes modal shell in `frontend/comfyui/ui/dialogs/ModalShell.js`.
 */

import { $el } from '/scripts/ui.js';
import { injectDialogStyles } from './DialogStyles.js';
import { ModalAppearance } from './ModalAppearance.js';
import { ModalDismissal } from './ModalDismissal.js';
import { mountModalLayer } from './ModalLayer.js';
import type { AppearanceSource } from '../surface/appearance/AppearanceState.js';

export interface ModalAdapter {
  getDocument?(): Document | null;
  getWindow?(): Window | null;
}

export interface ModalShellElements {
  overlay?: HTMLElement;
  dialog?: HTMLElement;
  titleEl?: HTMLElement;
  closeButton?: HTMLButtonElement;
  content?: HTMLElement;
  descriptionEl?: HTMLElement;
  errorEl?: HTMLElement;
  footerMeta?: HTMLElement;
  footer?: HTMLElement;
  confirmButton?: HTMLButtonElement;
  cancelButton?: HTMLButtonElement;
  actions?: HTMLElement;
}

export type ModalContent = string | Node | null | undefined;

export interface ModalOpenOptions {
  title?: string;
  description?: string | string[];
  body?: ModalContent | ModalContent[];
  footerMeta?: Node | Array<Node | null | undefined>;
  confirmLabel?: string;
  cancelLabel?: string;
  confirmClassName?: string;
  cancelResult?: unknown;
  allowEscapeClose?: boolean;
  allowOverlayClose?: boolean;
  showCancel?: boolean;
  onConfirm?: (() => void) | null;
  initialFocus?: (() => FocusableElement | null | undefined) | null;
}

interface ActiveModalOptions {
  cancelResult?: unknown;
  allowEscapeClose?: boolean;
  allowOverlayClose?: boolean;
  onConfirm?: (() => void) | null;
}

interface ModalShellOptions {
  appearanceSource?: AppearanceSource;
  adapter?: ModalAdapter | null;
  variantClassName?: string;
  dialogClassName?: string;
}

type FocusableElement = HTMLElement & { select?: () => void };

/**
 * Coordinate shared modal shell behavior for SugarCubes dialogs.
 */
export class ModalShell {
  private readonly appearance: ModalAppearance;
  private readonly documentRef: Document | null;
  private readonly windowRef: Window | null;
  private readonly variantClassName: string;
  private readonly dialogClassName: string;
  elements: ModalShellElements;
  private resolver: ((result: unknown) => void) | null;
  private keydownHandler: ((event: KeyboardEvent) => void) | null;
  private options: ActiveModalOptions;
  private focusTarget: (() => FocusableElement | null | undefined) | null;
  private previousActiveElement: HTMLElement | null;
  private readonly dismissal: ModalDismissal;
  /** Reserve Escape before native document handlers while respecting child popup ownership. */
  private readonly captureEscape = (event: KeyboardEvent): void => {
    this.dismissal.captureEscape(event);
  };

  /** Bind presentation lifetimes and one cancellation policy to the host document. */
  constructor({
    adapter,
    appearanceSource,
    variantClassName = '',
    dialogClassName = '',
  }: ModalShellOptions = {}) {
    this.appearance = new ModalAppearance(appearanceSource);
    this.documentRef = adapter?.getDocument?.() || null;
    this.windowRef = adapter?.getWindow?.() || null;
    this.variantClassName = variantClassName || '';
    this.dialogClassName = dialogClassName || '';
    this.elements = {};
    this.resolver = null;
    this.keydownHandler = null;
    this.options = {};
    this.focusTarget = null;
    this.previousActiveElement = null;
    this.dismissal = new ModalDismissal(
      this.documentRef,
      () => this.elements.dialog,
      () => this.options,
      () => this.close(this.options.cancelResult),
    );
  }

  /** Create persistent controls with explicit host-independent accessibility attributes. */
  ensureElements(): void {
    if (this.elements.overlay) {
      return;
    }
    injectDialogStyles(this.documentRef);
    const overlay = $el('div.sugarcubes-modal-overlay');
    if (this.variantClassName) {
      overlay.classList.add(this.variantClassName);
    }
    const dialogClassNames = ['sugarcubes-modal', 'p-dialog', 'p-component'];
    if (this.dialogClassName) {
      dialogClassNames.push(this.dialogClassName);
    }
    const dialog = $el('div', { className: dialogClassNames.join(' ') });
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');
    const titleEl = $el('span.p-dialog-title');
    const closeButton = $el('button.p-button.p-component.sugarcubes-modal__close', {
      type: 'button',
      title: 'Close',
    }) as HTMLButtonElement;
    closeButton.setAttribute('aria-label', 'Close dialog');
    const header = $el('div.p-dialog-header', [titleEl, closeButton]);
    const descriptionEl = $el('div.sugarcubes-modal__description');
    const content = $el('div.p-dialog-content');
    const errorEl = $el('div.sugarcubes-modal__error');
    errorEl.setAttribute('role', 'alert');
    errorEl.setAttribute('aria-live', 'polite');
    const footerMeta = $el('div.sugarcubes-modal__footer-meta');
    const cancelButton = $el('button.p-button.p-component.p-button-text.p-button-secondary', {
      type: 'button',
      textContent: 'Cancel',
    }) as HTMLButtonElement;
    const confirmButton = $el('button.p-button.p-component', {
      type: 'button',
      textContent: 'Confirm',
    }) as HTMLButtonElement;
    const actions = $el('div.sugarcubes-modal__footer-actions', [cancelButton, confirmButton]);
    const footer = $el('div.p-dialog-footer.sugarcubes-modal__footer', [footerMeta, actions]);

    content.append(descriptionEl, errorEl);
    dialog.append(header, content, footer);
    overlay.appendChild(dialog);
    this.documentRef?.body?.appendChild(overlay);

    overlay.addEventListener('pointerdown', (event) => this.dismissal.press(event), true);
    overlay.addEventListener('click', (event) => this.dismissal.outside(event));
    closeButton.addEventListener('click', () => this.dismissal.dismiss());
    cancelButton.addEventListener('click', () => this.dismissal.dismiss());
    confirmButton.addEventListener('click', () => {
      this.options.onConfirm?.();
    });

    this.elements = {
      overlay,
      dialog,
      titleEl,
      closeButton,
      content,
      descriptionEl,
      errorEl,
      footerMeta,
      footer,
      confirmButton,
      cancelButton,
      actions,
    };
  }

  /** Open an owned, themed dialog while preserving the caller's form and result contract. */
  open({
    title = '',
    description = [],
    body = [],
    footerMeta = [],
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    confirmClassName = '',
    cancelResult = null,
    allowEscapeClose = true,
    allowOverlayClose = true,
    showCancel = true,
    onConfirm = null,
    initialFocus = null,
  }: ModalOpenOptions = {}): Promise<unknown> {
    this.ensureElements();
    const { overlay, dialog, content, titleEl, confirmButton, cancelButton } = this.elements;
    if (!overlay || !dialog || !content || !titleEl || !confirmButton || !cancelButton) {
      return Promise.resolve(cancelResult);
    }
    if (this.resolver) {
      this.close(this.options.cancelResult);
    }

    this.options = {
      cancelResult,
      allowEscapeClose,
      allowOverlayClose,
      onConfirm,
    };
    this.focusTarget = typeof initialFocus === 'function' ? initialFocus : null;
    const activeElement = this.documentRef?.activeElement;
    this.previousActiveElement = activeElement instanceof HTMLElement ? activeElement : null;

    if (this.documentRef) mountModalLayer(this.documentRef, overlay);
    overlay.classList.add('is-visible');
    this.appearance.open(overlay);
    overlay.style.display = 'flex';
    titleEl.textContent = title || '';
    dialog.setAttribute('aria-label', title || 'SugarCubes');
    this.setDescription(description);
    this.setBody(body);
    this.setFooterMeta(footerMeta);
    this.setError('');
    this.setButtonClass(confirmButton, confirmClassName);
    confirmButton.textContent = confirmLabel || 'Confirm';
    confirmButton.dataset.allowConfirm = 'true';
    cancelButton.textContent = cancelLabel || 'Cancel';
    cancelButton.style.display = showCancel ? '' : 'none';
    this.setBusy(false);

    const onKeydown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || !this.dismissal.isFrontmost()) return;
      if (event.key === 'Escape') {
        this.dismissal.escape(event);
        return;
      }
      if (event.key === 'Enter' && !event.defaultPrevented) {
        const target = event.target;
        if (typeof HTMLTextAreaElement !== 'undefined' && target instanceof HTMLTextAreaElement) {
          return;
        }
        if (this.elements.confirmButton?.disabled) {
          return;
        }
        const tagName = target instanceof Element ? target.tagName.toLowerCase() : '';
        if (tagName === 'button') {
          return;
        }
        event.preventDefault();
        this.options.onConfirm?.();
      }
    };
    this.windowRef?.addEventListener?.('keydown', onKeydown);
    this.windowRef?.addEventListener?.('keydown', this.captureEscape, true);
    this.keydownHandler = onKeydown;

    this.focusInitialTarget();

    return new Promise<unknown>((resolve) => {
      this.resolver = resolve;
    });
  }

  /** Release visible dialog resources and restore focus before resolving the caller. */
  close(result: unknown): void {
    if (!this.elements.overlay) {
      return;
    }
    if (this.keydownHandler) {
      this.windowRef?.removeEventListener?.('keydown', this.keydownHandler);
      this.windowRef?.removeEventListener?.('keydown', this.captureEscape, true);
      this.keydownHandler = null;
    }
    this.elements.overlay.classList.remove('is-visible');
    this.appearance.close();
    this.elements.overlay.style.display = 'none';
    this.options = {};
    this.dismissal.reset();
    this.focusTarget = null;
    const focusTarget = this.previousActiveElement;
    this.previousActiveElement = null;
    focusTarget?.focus?.();
    if (this.resolver) {
      const resolve = this.resolver;
      this.resolver = null;
      resolve(result);
    }
  }

  setDescription(description: string | string[]): void {
    if (!this.elements.descriptionEl) {
      return;
    }
    const lines = (Array.isArray(description) ? description : [description])
      .map((value) => (typeof value === 'string' ? value : ''))
      .filter(Boolean);
    const nodes = lines.map((value) => $el('p', { textContent: value }));
    this.elements.descriptionEl.replaceChildren(...nodes);
    this.elements.descriptionEl.style.display = nodes.length ? '' : 'none';
  }

  setBody(body: ModalContent | ModalContent[]): void {
    if (!this.elements.content || !this.elements.descriptionEl || !this.elements.errorEl) {
      return;
    }
    const nodes = Array.isArray(body) ? body : [body];
    const filtered = nodes.filter((entry): entry is string | Node => Boolean(entry));
    const dynamicNodes = filtered.map((entry) => {
      if (typeof entry === 'string') {
        return $el('p', { textContent: entry });
      }
      return entry;
    });
    this.elements.content.replaceChildren(
      this.elements.descriptionEl,
      this.elements.errorEl,
      ...dynamicNodes,
    );
  }

  setFooterMeta(content: Node | Array<Node | null | undefined>): void {
    if (!this.elements.footerMeta || !this.elements.footer) {
      return;
    }
    const nodes = Array.isArray(content) ? content : [content];
    const filtered = nodes.filter((entry): entry is Node => entry instanceof Node);
    this.elements.footerMeta.replaceChildren(...filtered);
    this.elements.footerMeta.style.display = filtered.length ? '' : 'none';
    this.elements.footer.classList.toggle('sugarcubes-modal__footer--meta', filtered.length > 0);
  }

  setError(message: unknown): void {
    if (!this.elements.errorEl) {
      return;
    }
    const text = typeof message === 'string' ? message.trim() : '';
    this.elements.errorEl.textContent = text;
    this.elements.errorEl.style.display = text ? '' : 'none';
  }

  /** Keep all cancellation routes and acceptance controls aligned during a transaction. */
  setBusy(busy: unknown): void {
    const isBusy = Boolean(busy);
    this.dismissal.setBusy(isBusy);
    if (this.elements.closeButton) this.elements.closeButton.disabled = isBusy;
    if (this.elements.confirmButton) {
      const allowConfirm = this.elements.confirmButton.dataset.allowConfirm !== 'false';
      this.elements.confirmButton.disabled = isBusy || !allowConfirm;
      this.elements.confirmButton.dataset.busy = isBusy ? 'true' : 'false';
    }
    if (this.elements.cancelButton) {
      this.elements.cancelButton.disabled = isBusy;
    }
    if (this.elements.dialog) {
      this.elements.dialog.classList.toggle('is-busy', isBusy);
    }
  }

  /** Retain acceptance eligibility without re-enabling a busy transaction. */
  setConfirmEnabled(enabled: boolean): void {
    if (!this.elements.confirmButton) {
      return;
    }
    this.elements.confirmButton.dataset.allowConfirm = enabled ? 'true' : 'false';
    this.elements.confirmButton.disabled = this.dismissal.busy || !enabled;
  }

  focusInitialTarget(): void {
    const explicit = this.focusTarget?.();
    if (explicit?.focus) {
      explicit.focus();
      explicit.select?.();
      return;
    }
    this.elements.confirmButton?.focus?.();
  }

  setButtonClass(button: HTMLButtonElement | null | undefined, className: string): void {
    if (!button) {
      return;
    }
    const base = ['p-button', 'p-component'];
    if (button === this.elements.cancelButton) {
      base.push('p-button-text', 'p-button-secondary');
    }
    if (className) {
      base.push(className);
    }
    button.className = base.join(' ');
  }
}
