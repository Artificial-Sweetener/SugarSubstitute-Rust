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
/** Present Substitute-style seed split controls while Comfy owns values and execution modes. */
import type { NativeNumericField } from './ComfyNativeNumericField.js';
import type { NativeSeedControl } from './ComfyNativeSeedControl.js';
import type { SeedMode } from '../../graph/ComfySeedMode.js';

export interface NativeSeedField {
  numeric: NativeNumericField;
  control: NativeSeedControl;
}

interface SeedBinding {
  field: NativeSeedField;
  synchronize(): void;
  release(): void;
}

const modeLabels: Readonly<Record<SeedMode, string>> = {
  fixed: 'Use Current',
  randomize: 'Randomize',
  increment: 'Increment',
  decrement: 'Decrement',
};

const modeIcons: Readonly<Record<SeedMode, string>> = {
  fixed: 'locked',
  randomize: 'game-die',
  increment: 'add',
  decrement: 'subtract',
};

/** Resolve the pinned Microsoft asset for each supported seed operation. */
function seedIconImage(mode: SeedMode): string {
  const url = new URL(`../../../../icons/seed/${modeIcons[mode]}.svg`, import.meta.url);
  return `url("${url.href}")`;
}

/** Own seed accessories separately from native numeric entry and general spinner styling. */
export class NativeSeedControls {
  readonly #cards = new Map<HTMLElement, Map<HTMLInputElement, SeedBinding>>();

  /** Retain controls across native redraws and release replaced inputs and detached cards. */
  reconcile(root: HTMLElement, fields: readonly NativeSeedField[]): void {
    for (const [card, bindings] of this.#cards) {
      if (!card.isConnected) {
        for (const binding of bindings.values()) binding.release();
        this.#cards.delete(card);
      }
    }
    const bindings = this.#cards.get(root) ?? new Map<HTMLInputElement, SeedBinding>();
    this.#cards.set(root, bindings);
    for (const [input, binding] of bindings) {
      if (
        !fields.some(
          ({ numeric }) =>
            numeric.input === input && numeric.spinner === binding.field.numeric.spinner,
        )
      ) {
        binding.release();
        bindings.delete(input);
      }
    }
    for (const field of fields) {
      const existing = bindings.get(field.numeric.input);
      if (existing) {
        existing.field = field;
        existing.synchronize();
      } else bindings.set(field.numeric.input, this.#bind(field));
    }
  }

  /** Restore native controls without changing any seed value or mode. */
  clear(): void {
    for (const bindings of this.#cards.values())
      for (const binding of bindings.values()) binding.release();
    this.#cards.clear();
  }

  /** Attach an accessible quick-toggle button and a separate supported-mode menu. */
  #bind(field: NativeSeedField): SeedBinding {
    const { spinner } = field.numeric;
    const documentRef = spinner.ownerDocument;
    const split = documentRef.createElement('span');
    split.dataset.scSeedSplit = '';
    const primary = documentRef.createElement('button');
    primary.type = 'button';
    primary.dataset.scSeedToggle = '';
    const icon = documentRef.createElement('span');
    icon.setAttribute('aria-hidden', 'true');
    icon.dataset.scSeedIcon = '';
    primary.append(icon);
    const menuButton = documentRef.createElement('button');
    menuButton.type = 'button';
    menuButton.dataset.scSeedMenu = '';
    menuButton.setAttribute('aria-label', `${field.numeric.label} options`);
    menuButton.setAttribute('aria-haspopup', 'menu');
    menuButton.setAttribute('aria-expanded', 'false');
    const chevron = documentRef.createElement('span');
    chevron.setAttribute('aria-hidden', 'true');
    chevron.className = 'icon-[lucide--chevron-down]';
    menuButton.append(chevron);
    const menu = documentRef.createElement('div');
    menu.dataset.scSeedOptions = '';
    menu.setAttribute('role', 'menu');
    menu.hidden = true;
    menu.setAttribute('popover', 'manual');
    split.append(primary, menuButton, menu);
    spinner.dataset.scSeedBox = '';
    let selectedModes = '';
    const close = (): void => {
      if (typeof menu.hidePopover === 'function') menu.hidePopover();
      menu.hidden = true;
      menuButton.setAttribute('aria-expanded', 'false');
    };
    const choose = (mode: SeedMode): void => {
      if (binding.field.numeric.disabled()) return;
      binding.field.control.writeMode(mode);
      binding.synchronize();
      close();
      primary.focus();
    };
    const toggle = (): void =>
      choose(binding.field.control.readMode() === 'randomize' ? 'fixed' : 'randomize');
    const expand = (): void => {
      if (binding.field.numeric.disabled()) return;
      if (!menu.hidden) {
        close();
        return;
      }
      menu.hidden = false;
      menuButton.setAttribute('aria-expanded', 'true');
      if (typeof menu.showPopover === 'function') {
        const bounds = spinner.getBoundingClientRect();
        menu.style.position = 'fixed';
        menu.style.margin = '0';
        menu.style.right = 'auto';
        menu.style.bottom = 'auto';
        menu.style.left = `${Math.max(4, bounds.right - 158)}px`;
        menu.style.top = `${Math.min(bounds.bottom + 4, (documentRef.defaultView?.innerHeight ?? bounds.bottom + 164) - 164)}px`;
        menu.showPopover();
        const popup = menu.getBoundingClientRect();
        if (popup.bottom > (documentRef.defaultView?.innerHeight ?? popup.bottom))
          menu.style.top = `${Math.max(4, bounds.top - popup.height - 4)}px`;
      }
      menu.querySelector<HTMLButtonElement>('[aria-checked="true"]')?.focus();
    };
    const outside = (event: Event): void => {
      if (event.target instanceof Node && !split.contains(event.target)) close();
    };
    const key = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') {
        close();
        menuButton.focus();
        event.stopPropagation();
      }
      if (!menu.hidden && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
        const buttons = [...menu.querySelectorAll<HTMLButtonElement>('button')];
        const index = buttons.findIndex((button) => button === documentRef.activeElement);
        buttons[
          (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length
        ]?.focus();
        event.preventDefault();
        event.stopPropagation();
      }
    };
    const binding: SeedBinding = {
      field,
      synchronize: () => {
        const control = binding.field.control;
        const mode = control.readMode();
        const disabled = binding.field.numeric.disabled();
        primary.disabled = disabled;
        menuButton.disabled = disabled;
        primary.setAttribute(
          'aria-label',
          `${binding.field.numeric.label}: ${modeLabels[mode]}; switch to ${mode === 'randomize' ? 'fixed' : 'randomize'}`,
        );
        primary.title = modeLabels[mode];
        primary.dataset.scSeedMode = mode;
        const image = seedIconImage(mode);
        if (icon.style.getPropertyValue('--sc-seed-icon') !== image)
          icon.style.setProperty('--sc-seed-icon', image);
        const nextModes = control.modes.join(',');
        if (selectedModes !== nextModes) {
          selectedModes = nextModes;
          menu.replaceChildren(
            ...control.modes.map((value) => {
              const item = documentRef.createElement('button');
              item.type = 'button';
              item.setAttribute('role', 'menuitemradio');
              item.dataset.scSeedOption = value;
              const itemIcon = documentRef.createElement('span');
              itemIcon.dataset.scSeedIcon = '';
              itemIcon.setAttribute('aria-hidden', 'true');
              itemIcon.style.setProperty('--sc-seed-icon', seedIconImage(value));
              const label = documentRef.createElement('span');
              label.textContent = modeLabels[value];
              item.append(itemIcon, label);
              item.addEventListener('click', () => choose(value));
              return item;
            }),
          );
        }
        for (const item of menu.querySelectorAll<HTMLButtonElement>('button')) {
          item.setAttribute('aria-checked', String(item.dataset.scSeedOption === mode));
          item.disabled = disabled;
        }
        if (disabled) close();
      },
      release: () => {
        unsubscribe();
        split.remove();
        delete spinner.dataset.scSeedBox;
        documentRef.removeEventListener('pointerdown', outside, true);
        documentRef.removeEventListener('scroll', close, true);
        primary.removeEventListener('click', toggle);
        menuButton.removeEventListener('click', expand);
        split.removeEventListener('keydown', key);
      },
    };
    let unsubscribe = (): void => undefined;
    primary.addEventListener('click', toggle);
    menuButton.addEventListener('click', expand);
    split.addEventListener('keydown', key);
    documentRef.addEventListener('pointerdown', outside, true);
    documentRef.addEventListener('scroll', close, true);
    spinner.append(split);
    try {
      binding.synchronize();
      unsubscribe = field.numeric.subscribe(binding.synchronize);
    } catch (error: unknown) {
      binding.release();
      throw error;
    }
    return binding;
  }
}
