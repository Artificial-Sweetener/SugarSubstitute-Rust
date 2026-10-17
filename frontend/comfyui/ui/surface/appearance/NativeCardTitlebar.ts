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
/** Delegate themed titlebar gestures to existing native collapse and activation controls. */

interface HeaderBinding {
  header: HTMLElement;
  collapse: HTMLButtonElement | null;
  activation: HTMLInputElement | null;
  original: Map<string, string | null>;
  controls: Map<HTMLButtonElement, Map<string, string | null>>;
  release(): void;
}

/** Own reversible header interaction wiring without owning node state or layout. */
export class NativeCardTitlebar {
  readonly #bindings = new Map<HTMLElement, HeaderBinding>();

  /** Reconcile the exact native header and its rendered collapse state. */
  reconcile(
    root: HTMLElement,
    header: HTMLElement | null,
    collapsed: boolean,
    hasContent = true,
  ): void {
    for (const [card, binding] of this.#bindings) {
      if (!card.isConnected || (card === root && binding.header !== header)) {
        binding.release();
        this.#bindings.delete(card);
      }
    }
    if (!header) return;
    let binding = this.#bindings.get(root);
    if (!binding) {
      binding = bindHeader(root, header);
      this.#bindings.set(root, binding);
    }
    binding.collapse = header.querySelector<HTMLButtonElement>(
      '[data-testid="node-collapse-button"]',
    );
    binding.activation = header.querySelector<HTMLInputElement>(
      '[data-cube-card-activation] input',
    );
    const canActivate = binding.activation !== null && !binding.activation.disabled;
    const canCollapse = hasContent && binding.collapse !== null && !binding.collapse.disabled;
    const projected = root.closest('.sugarcubes-cube-face__node-card') !== null;
    const action = canActivate ? 'activation' : canCollapse && projected ? 'collapse' : null;
    if (canActivate || canCollapse) header.dataset.scTitlebarControls = '';
    else delete header.dataset.scTitlebarControls;
    const title = header.querySelector('[data-testid="node-title"]')?.textContent?.trim() ?? 'node';
    header.dataset.scTitlebarCollapsed = String(collapsed);
    if (action) {
      header.dataset.scTitlebarAction = action;
      header.setAttribute('role', 'button');
      header.tabIndex = 0;
      header.setAttribute(
        'aria-label',
        `${action === 'activation' ? (binding.activation?.checked ? 'Disable' : 'Enable') : collapsed ? 'Expand' : 'Collapse'} ${title}`,
      );
      if (action === 'collapse') header.setAttribute('aria-expanded', String(!collapsed));
      else header.removeAttribute('aria-expanded');
    } else {
      header.removeAttribute('data-sc-titlebar-action');
      restoreAttributes(header, binding.original);
    }
    if (binding.collapse) {
      if (!binding.controls.has(binding.collapse))
        binding.controls.set(
          binding.collapse,
          new Map(
            ['aria-label', 'aria-expanded', 'hidden'].map((name) => [
              name,
              binding.collapse?.getAttribute(name) ?? null,
            ]),
          ),
        );
      binding.collapse.hidden = !hasContent;
      binding.collapse.setAttribute('aria-label', `${collapsed ? 'Expand' : 'Collapse'} ${title}`);
      binding.collapse.setAttribute('aria-expanded', String(!collapsed));
    }
  }

  /** Release annotations and listeners when themed presentation ends. */
  clear(): void {
    for (const binding of this.#bindings.values()) binding.release();
    this.#bindings.clear();
  }
}

/** Wire one stable header while retaining all existing child control callbacks. */
function bindHeader(root: HTMLElement, header: HTMLElement): HeaderBinding {
  const original = new Map(
    ['role', 'tabindex', 'aria-label', 'aria-expanded'].map((name) => [
      name,
      header.getAttribute(name),
    ]),
  );
  const controlAttributes = new Map<HTMLButtonElement, Map<string, string | null>>();
  const collapse = header.querySelector<HTMLButtonElement>('[data-testid="node-collapse-button"]');
  if (collapse)
    controlAttributes.set(
      collapse,
      new Map(
        ['aria-label', 'aria-expanded', 'hidden'].map((name) => [
          name,
          collapse.getAttribute(name),
        ]),
      ),
    );
  let pointer: { x: number; y: number } | null = null;
  let dragged = false;
  const binding: HeaderBinding = {
    header,
    collapse: null,
    activation: null,
    original,
    controls: controlAttributes,
    release: () => {
      for (const [type, listener] of listeners) header.removeEventListener(type, listener);
      for (const name of ['action', 'collapsed', 'controls'])
        header.removeAttribute(`data-sc-titlebar-${name}`);
      restoreAttributes(header, original);
      for (const [button, attributes] of controlAttributes) restoreAttributes(button, attributes);
    },
  };
  const activate = (): void => {
    const action = header.dataset.scTitlebarAction;
    const control =
      action === 'activation'
        ? binding.activation
        : action === 'collapse'
          ? binding.collapse
          : null;
    if (control && !control.disabled && !control.hidden) control.click();
  };
  const listeners: Array<[string, EventListener]> = [
    [
      'pointerdown',
      (event) => {
        dragged = false;
        if (event instanceof MouseEvent) pointer = { x: event.clientX, y: event.clientY };
        if (root.dataset.cubeFacePresentation === 'true' && !isChildControl(event.target, header))
          event.stopPropagation();
      },
    ],
    [
      'pointerup',
      (event) => {
        if (pointer && event instanceof MouseEvent)
          dragged = Math.hypot(event.clientX - pointer.x, event.clientY - pointer.y) > 4;
        pointer = null;
      },
    ],
    [
      'click',
      (event) => {
        if (!header.dataset.scTitlebarAction || isChildControl(event.target, header)) return;
        if (!dragged) activate();
        dragged = false;
        event.stopPropagation();
      },
    ],
    [
      'keydown',
      (event) => {
        if (
          !(event instanceof KeyboardEvent) ||
          !header.dataset.scTitlebarAction ||
          event.target !== header ||
          !['Enter', ' '].includes(event.key)
        )
          return;
        event.preventDefault();
        event.stopPropagation();
        activate();
      },
    ],
  ];
  for (const [type, listener] of listeners) header.addEventListener(type, listener);
  return binding;
}

/** Preserve controls that break out of ordinary title-row gestures. */
function isChildControl(target: EventTarget | null, header: HTMLElement): boolean {
  return (
    target instanceof Element &&
    target !== header &&
    target.closest(
      'button, input, textarea, select, a, label, [contenteditable="true"], [role="switch"], [role="combobox"]',
    ) !== null
  );
}

/** Restore host accessibility attributes without manufacturing new persisted state. */
function restoreAttributes(
  element: HTMLElement,
  attributes: ReadonlyMap<string, string | null>,
): void {
  for (const [name, value] of attributes) {
    if (value === null) element.removeAttribute(name);
    else element.setAttribute(name, value);
  }
}
