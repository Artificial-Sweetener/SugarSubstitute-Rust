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
/** Adapt standard host settings switches while Comfy retains persistence and change callbacks. */
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
import { createActivationSwitch } from '../surface/appearance/ControlFactory.js';
import { readOwnedVisualStyle } from '../surface/appearance/VisualStyleBoundary.js';

interface SettingSwitchBinding {
  refresh(): void;
  release(): void;
}

/** Own reversible Spectrum switch presentation only inside identified standard settings rows. */
export class SettingsSwitchControls {
  readonly #bindings = new Map<HTMLInputElement, SettingSwitchBinding>();
  #observer: HostSurfaceObserver | null = null;

  /** Bind the exact settings mount without observing unrelated portals or graph surfaces. */
  constructor(private readonly root: HTMLElement) {}

  /** Preserve native inputs and callbacks while presenting the selected system's activation control. */
  reconcile(styled: boolean): void {
    if (!styled || readOwnedVisualStyle(this.root.ownerDocument) !== 'spectrum') {
      this.clear();
      return;
    }
    for (const [input, binding] of this.#bindings) {
      if (this.root.contains(input) && input.closest('[data-sc-setting-kind="standard"]')) continue;
      binding.release();
      this.#bindings.delete(input);
    }
    for (const input of this.root.querySelectorAll<HTMLInputElement>(
      '[data-sc-setting-kind="standard"] [data-pc-name="toggleswitch"] input[type="checkbox"]',
    )) {
      let binding = this.#bindings.get(input);
      if (!binding) {
        binding = this.#bind(input);
        this.#bindings.set(input, binding);
      }
      binding.refresh();
    }
    this.#observer ??= new HostSurfaceObserver(
      this.root,
      '[data-sc-setting-kind="standard"]',
      () => this.reconcile(true),
      {
        content: true,
        attributes: ['aria-checked', 'checked', 'disabled'],
        exclude: '.sc-settings-owned',
      },
    );
    this.#observer.discardPending();
  }

  /** Restore the original host controls without changing any saved setting. */
  clear(): void {
    this.#observer?.disconnect();
    this.#observer = null;
    for (const binding of this.#bindings.values()) binding.release();
    this.#bindings.clear();
  }

  /** Forward exactly one gesture to the retained native switch and mirror accepted state. */
  #bind(input: HTMLInputElement): SettingSwitchBinding {
    const nativeRoot = input.closest<HTMLElement>('[data-pc-name="toggleswitch"]');
    if (!nativeRoot?.parentElement) throw new Error('Standard settings switch is not mounted.');
    const presentation = createActivationSwitch(input.ownerDocument);
    presentation.root.classList.add('sc-settings-owned');
    presentation.root.dataset.scSettingsSwitch = '';
    const hidden = nativeRoot.hidden;
    const focused = input.ownerDocument.activeElement === input;
    nativeRoot.hidden = true;
    nativeRoot.dataset.scSettingsSwitchNative = '';
    nativeRoot.before(presentation.root);
    const refresh = (): void => {
      const labelledBy = input
        .getAttribute('aria-labelledby')
        ?.split(/\s+/)
        .map((id) => input.ownerDocument.getElementById(id)?.textContent?.trim() ?? '')
        .filter(Boolean)
        .join(' ');
      const label =
        input.getAttribute('aria-label') ??
        (labelledBy || undefined) ??
        input.closest('[data-setting-id]')?.querySelector('.form-label')?.textContent?.trim() ??
        'Setting';
      presentation.control.disabled = input.disabled;
      presentation.synchronize(input.checked, label, '');
    };
    const change = (event: Event): void => {
      event.stopPropagation();
      if (!input.disabled && input.checked !== presentation.control.checked) input.click();
      refresh();
    };
    const retainGesture = (event: Event): void => event.stopPropagation();
    presentation.control.addEventListener('change', change);
    presentation.root.addEventListener('pointerdown', retainGesture);
    presentation.root.addEventListener('click', retainGesture);
    input.addEventListener('change', refresh);
    refresh();
    if (focused) presentation.control.focus();
    return {
      refresh,
      release: () => {
        const restoreFocus = input.ownerDocument.activeElement === presentation.control;
        input.removeEventListener('change', refresh);
        presentation.control.removeEventListener('change', change);
        presentation.root.removeEventListener('pointerdown', retainGesture);
        presentation.root.removeEventListener('click', retainGesture);
        presentation.root.remove();
        nativeRoot.hidden = hidden;
        delete nativeRoot.dataset.scSettingsSwitchNative;
        if (restoreFocus) input.focus();
      },
    };
  }
}
