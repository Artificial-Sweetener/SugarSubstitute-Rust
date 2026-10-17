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
/** Adapt official Adobe Spectrum components and Spectrum 2 foundations at the browser boundary. */
import { ButtonBase } from '@spectrum-web-components/button/src/ButtonBase.js';
import '@spectrum-web-components/action-button/sp-action-button.js';
import '@spectrum-web-components/menu/sp-menu.js';
import '@spectrum-web-components/menu/sp-menu-item.js';
import { MenuItem } from '@spectrum-web-components/menu/src/MenuItem.js';
import '@spectrum-web-components/checkbox/sp-checkbox.js';
import '@spectrum-web-components/switch/sp-switch.js';
import { Switch } from '@spectrum-web-components/switch/src/Switch.js';
import { Slider } from '@spectrum-web-components/slider/src/Slider.js';
import { Tabs } from '@spectrum-web-components/tabs/src/Tabs.js';
import '@spectrum-web-components/tabs/sp-tab.js';
import { Textfield } from '@spectrum-web-components/textfield/src/Textfield.js';
import core from '@spectrum-web-components/theme/src/spectrum-two/theme.css.js';
import scale from '@spectrum-web-components/theme/src/spectrum-two/scale-medium.css.js';
import dark from '@spectrum-web-components/theme/src/spectrum-two/theme-dark.css.js';
import light from '@spectrum-web-components/theme/src/spectrum-two/theme-light.css.js';
import { spectrumRootTokens, spectrumSemanticTokens } from './SpectrumThemeTokens.js';
import type { NumericSliderControl } from './NativeNumericControls.js';
import type { BooleanCheckboxControl } from './NativeToggleRows.js';
import type { BooleanSegmentsControls } from './NativeBooleanSegments.js';
import type { FinderInputControl } from './FinderControls.js';
import type { ActivationSwitchControl } from './ActivationSwitchControl.js';

/** Preserve a focused activation control when its provider changes before Lit's first render. */
class SpectrumActivationSwitch extends Switch {
  /** Focus Adobe's native input only after it has connected and rendered. */
  override focus(options?: FocusOptions): void {
    void this.updateComplete
      .then(() => {
        if (this.isConnected) super.focus(options);
      })
      .catch((error: unknown) =>
        console.error('Spectrum activation switch could not receive focus.', { error }),
      );
  }

  /** Retain the active input across supported graph and masonry moves. */
  connectedMoveCallback(): void {}
}

/** Defer initial focus until Adobe has completed its first connected tabindex update. */
class SpectrumMenuItem extends MenuItem {
  /** Preserve the shared menu owner's requested focus through the initial Lit render. */
  override focus(): void {
    if (this.hasUpdated) {
      super.focus();
      return;
    }
    void this.updateComplete
      .then(() => {
        if (this.isConnected) super.focus();
      })
      .catch((error: unknown) =>
        console.error('Spectrum menu item could not receive focus.', { error }),
      );
  }
}

/** Retain Adobe's gesture owner while normalizing numeric state for native Comfy bindings. */
class SpectrumSlider extends Slider implements NumericSliderControl {
  /** Expose the actual Adobe component without leaking its API to widget bindings. */
  get root(): HTMLElement {
    return this;
  }
  /** Read Adobe's numeric value. */
  readValue(): number {
    return this.value;
  }
  /** Accept native numeric values without string coercion or synthetic host edits. */
  synchronize(
    minimum: number,
    maximum: number,
    step: number,
    disabled: boolean,
    value: number | null,
  ): void {
    this.min = minimum;
    this.max = maximum;
    this.step = step;
    this.disabled = disabled;
    if (value !== null) this.value = value;
  }
  /** Preserve pointer and focus ownership during state-preserving graph/masonry moves. */
  connectedMoveCallback(): void {}
}

/** Normalize Adobe's selection vocabulary for the shared tab interaction owner. */
class SpectrumTabGroup extends Tabs {
  /** Expose the selected child identity used by native segmented controls and workflow tabs. */
  get activeid(): string {
    return this.selected;
  }
  /** Project stable child ids into Adobe's value-based selection model. */
  set activeid(value: string) {
    for (const tab of this.querySelectorAll('sp-tab')) {
      tab.setAttribute('role', 'tab');
      tab.setAttribute('value', tab.id);
      tab.toggleAttribute('selected', tab.id === value);
    }
    this.selected = value;
  }
  /** Retain keyboard and focus state through supported native presentation moves. */
  connectedMoveCallback(): void {}
}

/** Expose the real text input through the finder control contract. */
class SpectrumFinderInput extends Textfield {
  /** Accept native and declarative focus requests before the first connected render. */
  override focus(): void {
    this.focusInput(false);
  }

  /** Wait for Adobe's connected render before focusing its real input. */
  focusInput(selectAll: boolean): void {
    void this.updateComplete
      .then(() => {
        if (!this.isConnected || this.closest('[hidden]')) return;
        this.control.setAttribute(
          'aria-label',
          this.getAttribute('aria-label') ?? 'Search masonry',
        );
        this.control.focus();
        if (selectAll) this.select();
      })
      .catch((error: unknown) =>
        console.error('Spectrum search control could not receive focus.', { error }),
      );
  }
  /** Narrow Adobe's single-line focus target after its first connected update. */
  get control(): HTMLInputElement {
    const input = this.focusElement;
    if (!(input instanceof HTMLInputElement))
      throw new TypeError('Spectrum search input is not mounted.');
    return input;
  }
  /** Preserve the field's value and focus through supported presentation moves. */
  connectedMoveCallback(): void {}
}

if (!customElements.get('sc-spectrum-slider'))
  customElements.define('sc-spectrum-slider', SpectrumSlider);
if (!customElements.get('sc-spectrum-tabs'))
  customElements.define('sc-spectrum-tabs', SpectrumTabGroup);
if (!customElements.get('sc-spectrum-search'))
  customElements.define('sc-spectrum-search', SpectrumFinderInput);
if (!customElements.get('sc-spectrum-menu-item'))
  customElements.define('sc-spectrum-menu-item', SpectrumMenuItem);
if (!customElements.get('sc-spectrum-switch'))
  customElements.define('sc-spectrum-switch', SpectrumActivationSwitch);

const base = spectrumRootTokens(core.cssText) + ';' + spectrumRootTokens(scale.cssText);
const palettes = {
  dark: spectrumRootTokens(dark.cssText),
  light: spectrumRootTokens(light.cssText),
};

/** Serialize only scoped official design tokens and the complete semantic role contract. */
export function spectrumTokenDeclarations(isDark: boolean): string {
  return base + ';' + (isDark ? palettes.dark : palettes.light) + ';' + spectrumSemanticTokens();
}

/** Create a quiet official Adobe action button for product toolbar commands. */
export function createSpectrumButton(document: Document): HTMLElement {
  const button = document.createElement('sp-action-button');
  if (!(button instanceof ButtonBase))
    throw new TypeError('Spectrum button registration is incompatible.');
  button.setAttribute('quiet', '');
  button.setAttribute('size', 's');
  return button;
}

/** Bind Adobe's slider without replacing the native Comfy value owner. */
export function createSpectrumSlider(document: Document): NumericSliderControl {
  const slider = document.createElement('sc-spectrum-slider');
  if (!(slider instanceof SpectrumSlider))
    throw new TypeError('Spectrum slider registration is incompatible.');
  slider.labelVisibility = 'none';
  slider.variant = 'filled';
  slider.setAttribute('size', 's');
  return slider;
}

/** Create Adobe's checkbox through the shared checked/disabled control contract. */
export function createSpectrumCheckbox(document: Document): BooleanCheckboxControl {
  const checkbox = document.createElement('sp-checkbox');
  checkbox.setAttribute('size', 's');
  return checkbox;
}

/** Delegate activation gestures and keyboard behavior to Adobe's actual switch. */
export function createSpectrumActivationSwitch(document: Document): ActivationSwitchControl {
  const control = document.createElement('sc-spectrum-switch');
  if (!(control instanceof SpectrumActivationSwitch))
    throw new TypeError('Spectrum switch registration is incompatible.');
  control.setAttribute('size', 's');
  control.emphasized = true;
  return {
    root: control,
    control,
    synchronize: (checked, actionLabel, stateLabel) => {
      control.checked = checked;
      control.textContent = stateLabel;
      control.setAttribute('aria-label', actionLabel);
      void control.updateComplete
        .then(() => {
          control.focusElement.setAttribute('aria-label', actionLabel);
        })
        .catch((error: unknown) =>
          console.error('Spectrum activation switch could not update its accessible name.', {
            error,
          }),
        );
    },
  };
}

/** Create Adobe's tab group with stable identities and its own keyboard navigation. */
export function createSpectrumTablist(document: Document): HTMLElement & { activeid: string } {
  const group = document.createElement('sc-spectrum-tabs');
  if (!(group instanceof SpectrumTabGroup))
    throw new TypeError('Spectrum tabs registration is incompatible.');
  group.auto = true;
  group.compact = true;
  group.setAttribute('size', 's');
  return group;
}

let nextSegment = 0;

/** Create mutually exclusive Adobe controls while Comfy retains the boolean value. */
export function createSpectrumSegments(document: Document): BooleanSegmentsControls {
  const group = createSpectrumTablist(document);
  const off = document.createElement('sp-tab');
  const on = document.createElement('sp-tab');
  off.id = `sc-spectrum-boolean-${++nextSegment}`;
  on.id = `sc-spectrum-boolean-${++nextSegment}`;
  off.value = off.id;
  on.value = on.id;
  return { group, segments: [off, on], release: () => group.remove() };
}

/** Create an official single-line Adobe input for the shared finder. */
export function createSpectrumFinderInput(document: Document): FinderInputControl {
  const input = document.createElement('sc-spectrum-search');
  if (!(input instanceof SpectrumFinderInput))
    throw new TypeError('Spectrum input registration is incompatible.');
  input.setAttribute('size', 's');
  input.label = 'Search masonry';
  return input;
}
