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
/** Bundle Microsoft's theme implementation at the third-party browser boundary. */
import { setTheme } from '@fluentui/web-components/theme/set-theme.js';
import { webDarkTheme, webLightTheme } from '@fluentui/tokens';
import { Slider } from '@fluentui/web-components/slider/class.js';
import { definition as SliderDefinition } from '@fluentui/web-components/slider/definition.js';
import { Button } from '@fluentui/web-components/button/class.js';
import { definition as ButtonDefinition } from '@fluentui/web-components/button/definition.js';
import { Checkbox } from '@fluentui/web-components/checkbox/class.js';
import { definition as CheckboxDefinition } from '@fluentui/web-components/checkbox/definition.js';
import { TextInput } from '@fluentui/web-components/text-input/class.js';
import { definition as TextInputDefinition } from '@fluentui/web-components/text-input/definition.js';
import type { FluentFinderInput } from './FluentFinderControls.js';
import type { FluentNumericSlider } from './NativeNumericControls.js';
import type { FluentBooleanCheckbox } from './NativeToggleRows.js';
import { adaptFluentSliderGeometry } from './FluentSliderGeometry.js';
import { createFluentSliderThumb } from './FluentSliderThumb.js';

/** Retain the official slider's bindings and stylesheet during connected presentation moves. */
class PresentationSlider extends Slider {
  /** Preserve mounted state while native layout observes the new parent width. */
  connectedMoveCallback(): void {}
}

/** Retain the official button's native state during connected presentation moves. */
class PresentationButton extends Button {
  /** Keep mounted bindings intact when its enclosing card changes presentation. */
  connectedMoveCallback(): void {}
}

/** Retain the official checkbox's form and value bindings during connected presentation moves. */
class PresentationCheckbox extends Checkbox {
  /** Keep mounted bindings intact when its enclosing card changes presentation. */
  connectedMoveCallback(): void {}
}

// FAST completes component registration asynchronously, before consumers can create controls.
if (!customElements.get('fluent-slider')) await PresentationSlider.define(SliderDefinition);
if (!customElements.get('fluent-button')) await PresentationButton.define(ButtonDefinition);
if (!customElements.get('fluent-checkbox')) await PresentationCheckbox.define(CheckboxDefinition);
if (!customElements.get('fluent-text-input')) await TextInput.define(TextInputDefinition);

/** Instantiate Microsoft's native text input through the narrow finder control boundary. */
export function createFluentFinderInput(documentRef: Document): FluentFinderInput {
  const input = documentRef.createElement('fluent-text-input');
  if (!(input instanceof TextInput))
    throw new TypeError('The registered Fluent input is incompatible.');
  return input;
}

/** Instantiate Microsoft's checkbox while Comfy retains the boolean value owner. */
export function createFluentBooleanCheckbox(documentRef: Document): FluentBooleanCheckbox {
  const checkbox = documentRef.createElement('fluent-checkbox');
  if (!(checkbox instanceof Checkbox))
    throw new TypeError('The registered Fluent checkbox has an incompatible implementation.');
  return checkbox;
}

/** Instantiate Microsoft's official slider without replacing Comfy's numeric value owner. */
export function createFluentNumericSlider(documentRef: Document): FluentNumericSlider {
  const slider = documentRef.createElement('fluent-slider');
  if (!(slider instanceof Slider))
    throw new TypeError('The registered Fluent slider has an incompatible implementation.');
  adaptFluentSliderGeometry(slider);
  slider.append(createFluentSliderThumb(documentRef));
  return slider;
}

/** Share Microsoft's motion timings across presentation lifecycles and themes. */
export const fluentMotion = {
  duration: Number.parseFloat(webDarkTheme.durationNormal),
  exitDuration: Number.parseFloat(webDarkTheme.durationFast),
  easing: webDarkTheme.curveDecelerateMid,
};

const fontFamilyBase = '"Segoe UI Variable", "Segoe UI", sans-serif';
const themes = {
  dark: { ...webDarkTheme, fontFamilyBase },
  light: { ...webLightTheme, fontFamilyBase },
};

/** Serialize trusted Microsoft tokens for scoped native-node inheritance. */
export function fluentTokenDeclarations(dark: boolean): string {
  return Object.entries(dark ? themes.dark : themes.light)
    .map(([name, value]) => `--${name}:${value};`)
    .join('');
}

/** Theme only a SugarCubes-owned container through Microsoft's official API. */
export function applyOwnedFluentTheme(element: HTMLElement, dark: boolean | null): void {
  setTheme(dark === null ? null : dark ? themes.dark : themes.light, element);
}
