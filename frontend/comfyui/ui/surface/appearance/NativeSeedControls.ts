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
import { nativeSeedMenuEntries, modeLabels } from './SeedMenuEntries.js';
import { seedIconImage } from './SeedActionIcons.js';
import { ContextMenu } from '../../menus/ContextMenu.js';

export interface NativeSeedField {
  numeric: NativeNumericField;
  control: NativeSeedControl;
}

interface SeedBinding {
  field: NativeSeedField;
  synchronize(): void;
  release(): void;
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
    const menu = new ContextMenu(documentRef);
    split.append(primary, menuButton);
    const retainInteraction = (event: Event): void => {
      event.stopPropagation();
    };
    for (const name of ['pointerdown', 'mousedown', 'click'] as const)
      split.addEventListener(name, retainInteraction);
    spinner.dataset.scSeedBox = '';
    let renderedMode = '';
    let renderedModes = '';
    const close = (): void => {
      menu.close();
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
      if (menu.isOpen()) {
        close();
        return;
      }
      menuButton.setAttribute('aria-expanded', 'true');
      const control = binding.field.control;
      menu.open({
        label: `${binding.field.numeric.label} options`,
        anchor: { kind: 'below', bounds: spinner.getBoundingClientRect() },
        trigger: menuButton,
        focusChecked: true,
        attributes: { 'data-sc-seed-options': '' },
        closed: () => menuButton.setAttribute('aria-expanded', 'false'),
        entries: nativeSeedMenuEntries(control, choose, () => {
          if (binding.field.numeric.disabled()) return;
          binding.field.control.reuse?.reuse();
          binding.synchronize();
          close();
          primary.focus();
        }),
      });
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
        if (menu.isOpen() && (mode !== renderedMode || control.modes.join(',') !== renderedModes))
          close();
        renderedMode = mode;
        renderedModes = control.modes.join(',');
        if (disabled) close();
      },
      release: () => {
        close();
        unsubscribe();
        split.remove();
        delete spinner.dataset.scSeedBox;
        primary.removeEventListener('click', toggle);
        menuButton.removeEventListener('click', expand);
        for (const name of ['pointerdown', 'mousedown', 'click'] as const)
          split.removeEventListener(name, retainInteraction);
      },
    };
    let unsubscribe = (): void => undefined;
    primary.addEventListener('click', toggle);
    menuButton.addEventListener('click', expand);
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
