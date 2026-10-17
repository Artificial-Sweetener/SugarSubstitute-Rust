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
import { webDarkTheme, webLightTheme, createDarkTheme, createLightTheme } from '@fluentui/tokens';
import {
  accentForeground,
  accentIndicatorColor,
  createAccentBrandPalette,
  mixAccentColor,
} from './AccentPalette.js';
import { Slider } from '@fluentui/web-components/slider/class.js';
import { definition as SliderDefinition } from '@fluentui/web-components/slider/definition.js';
import { Button } from '@fluentui/web-components/button/class.js';
import { definition as ButtonDefinition } from '@fluentui/web-components/button/definition.js';
import { Tab } from '@fluentui/web-components/tab/class.js';
import { definition as TabDefinition } from '@fluentui/web-components/tab/definition.js';
import { styles as TabStyles } from '@fluentui/web-components/tab/styles.js';
import { Tablist } from '@fluentui/web-components/tablist/class.js';
import { definition as TablistDefinition } from '@fluentui/web-components/tablist/definition.js';
import { styles as TablistStyles } from '@fluentui/web-components/tablist/styles.js';
import { Checkbox } from '@fluentui/web-components/checkbox/class.js';
import { definition as CheckboxDefinition } from '@fluentui/web-components/checkbox/definition.js';
import { TextInput } from '@fluentui/web-components/text-input/class.js';
import { definition as TextInputDefinition } from '@fluentui/web-components/text-input/definition.js';
import type { FinderInputControl } from './FinderControls.js';
import type { NumericSliderControl } from './NativeNumericControls.js';
import type { BooleanCheckboxControl } from './NativeToggleRows.js';
import type { BooleanSegmentsControls } from './NativeBooleanSegments.js';
import { adaptFluentSliderGeometry } from './FluentSliderGeometry.js';
import { createFluentSliderThumb } from './FluentSliderThumb.js';
import { css } from '@microsoft/fast-element';
import { workflowTabCss, workflowTablistCss } from '../frame/WorkflowTabStyles.js';
import { fluentThemeTokens } from './FluentThemeTokens.js';
import { themeTokenDeclarations } from './ThemeTokens.js';

/** Scope Substitute geometry to workflow tabs without changing output or boolean segments. */
class WorkflowTab extends Tab {}
/** Retain Microsoft's native focus group for the complete workflow-tab lifetime. */
class WorkflowTablist extends Tablist {}

/** Retain the official slider's bindings and stylesheet during connected presentation moves. */
class PresentationSlider extends Slider implements NumericSliderControl {
  /** Expose the official component through the shared numeric-control boundary. */
  get root(): HTMLElement {
    return this;
  }
  /** Normalize Microsoft's string value before it reaches native widget bindings. */
  readValue(): number {
    return Number(this.value);
  }
  /** Translate native numeric values into Microsoft's string-based properties. */
  synchronize(
    minimum: number,
    maximum: number,
    step: number,
    disabled: boolean,
    value: number | null,
  ): void {
    this.min = String(minimum);
    this.max = String(maximum);
    this.step = String(step);
    this.disabled = disabled;
    if (value !== null) this.value = String(value);
  }
  /** Preserve mounted state while native layout observes the new parent width. */
  connectedMoveCallback(): void {}
}

/** Retain the official button's native state during connected presentation moves. */
class PresentationButton extends Button {
  /** Keep mounted bindings intact when its enclosing card changes presentation. */
  connectedMoveCallback(): void {}
}

/** Retain the official tab's selection and focus during presentation moves. */
class PresentationTab extends Tab {
  /** Keep mounted bindings intact when the enclosing field changes presentation. */
  connectedMoveCallback(): void {}
}

/** Retain Microsoft's focus group through connected graph and masonry moves. */
class PresentationTablist extends Tablist {
  /** Preserve keyboard bindings while the same native field changes parent. */
  connectedMoveCallback(): void {}

  /** Leave teardown to the binding because Comfy parks native fields between presentation mounts. */
  override disconnectedCallback(): void {}

  /** Release Microsoft's bindings and focus group once the owning field adapter retires. */
  releasePresentation(): void {
    super.disconnectedCallback();
  }
}

/** Retain the official checkbox's form and value bindings during connected presentation moves. */
class PresentationCheckbox extends Checkbox {
  /** Keep mounted bindings intact when its enclosing card changes presentation. */
  connectedMoveCallback(): void {}
}

/** Keep finder focus synchronous after FAST mounts its input. */
class PresentationTextInput extends TextInput implements FinderInputControl {
  /** Apply the shared accessible name before focusing the real input. */
  focusInput(selectAll: boolean): void {
    this.control.setAttribute('aria-label', this.getAttribute('aria-label') ?? 'Search masonry');
    this.control.focus();
    if (selectAll) this.select();
  }
}

// FAST completes component registration asynchronously, before consumers can create controls.
if (!customElements.get('fluent-slider')) await PresentationSlider.define(SliderDefinition);
if (!customElements.get('fluent-button')) await PresentationButton.define(ButtonDefinition);
if (!customElements.get('fluent-tab')) await PresentationTab.define(TabDefinition);
if (!customElements.get('fluent-tablist')) await PresentationTablist.define(TablistDefinition);
if (!customElements.get('fluent-checkbox')) await PresentationCheckbox.define(CheckboxDefinition);
if (!customElements.get('fluent-text-input'))
  await PresentationTextInput.define(TextInputDefinition);
const workflowTabDefinition: typeof TabDefinition = {
  ...TabDefinition,
  name: 'sc-workflow-tab',
  styles: [
    TabStyles,
    css`
      ${workflowTabCss}
    `,
  ],
};
const workflowTablistDefinition: typeof TablistDefinition = {
  ...TablistDefinition,
  name: 'sc-workflow-tablist',
  styles: [
    TablistStyles,
    css`
      ${workflowTablistCss}
    `,
  ],
};
if (!customElements.get('sc-workflow-tab')) await WorkflowTab.define(workflowTabDefinition);
if (!customElements.get('sc-workflow-tablist'))
  await WorkflowTablist.define(workflowTablistDefinition);

/** Create an official workflow tablist without constructor-time document-create restrictions. */
export function createWorkflowTablist(): HTMLElement & { activeid: string } {
  return new WorkflowTablist();
}

/** Instantiate one workflow-specific official Fluent tab. */
export function createWorkflowTab(documentRef: Document): HTMLElement {
  return documentRef.createElement('sc-workflow-tab');
}

/** Instantiate Microsoft's native text input through the narrow finder control boundary. */
export function createFinderInputControl(documentRef: Document): FinderInputControl {
  const input = documentRef.createElement('fluent-text-input');
  if (!(input instanceof PresentationTextInput))
    throw new TypeError('The registered Fluent input is incompatible.');
  return input;
}

/** Instantiate Microsoft's checkbox while Comfy retains the boolean value owner. */
export function createBooleanCheckboxControl(documentRef: Document): BooleanCheckboxControl {
  const checkbox = documentRef.createElement('fluent-checkbox');
  if (!(checkbox instanceof Checkbox))
    throw new TypeError('The registered Fluent checkbox has an incompatible implementation.');
  return checkbox;
}

let nextBooleanSegmentId = 0;

/** Instantiate Microsoft's mutually exclusive tabs with their shared sliding accent indicator. */
export function createBooleanSegmentControls(documentRef: Document): BooleanSegmentsControls {
  const GroupConstructor = documentRef.defaultView?.customElements.get('fluent-tablist');
  if (!GroupConstructor) throw new TypeError('The Fluent tablist is not registered.');
  // Microsoft's focus-group constructor initializes host attributes before connection.
  const group = new GroupConstructor();
  group.setAttribute('size', 'small');
  group.setAttribute('appearance', 'subtle');
  const off = documentRef.createElement('fluent-tab');
  const on = documentRef.createElement('fluent-tab');
  if (!(group instanceof PresentationTablist) || !(off instanceof Tab) || !(on instanceof Tab))
    throw new TypeError('The registered Fluent tab controls have incompatible implementations.');
  off.id = `sc-boolean-${++nextBooleanSegmentId}`;
  on.id = `sc-boolean-${++nextBooleanSegmentId}`;
  off.slot = 'tab';
  on.slot = 'tab';
  return { group, segments: [off, on], release: () => group.releasePresentation() };
}

/** Instantiate Microsoft's official slider without replacing Comfy's numeric value owner. */
export function createFluentNumericSlider(documentRef: Document): Slider & NumericSliderControl {
  const slider = documentRef.createElement('fluent-slider');
  if (!(slider instanceof PresentationSlider))
    throw new TypeError('The registered Fluent slider has an incompatible implementation.');
  adaptFluentSliderGeometry(slider);
  slider.append(createFluentSliderThumb(documentRef));
  return slider;
}

const fontFamilyBase = '"Segoe UI Variable", "Segoe UI", sans-serif';
const themes = {
  dark: { ...webDarkTheme, fontFamilyBase },
  light: { ...webLightTheme, fontFamilyBase },
};
const appliedThemes = new WeakMap<HTMLElement, boolean | null>();

/** Serialize trusted Microsoft tokens for scoped native-node inheritance. */
export function fluentTokenDeclarations(dark: boolean): string {
  const theme = dark ? themes.dark : themes.light;
  return (
    Object.entries(theme)
      .map(([name, value]) => `--${name}:${value};`)
      .join('') + themeTokenDeclarations(fluentThemeTokens(theme))
  );
}

/** Apply accepted palette changes once while respecting the native API's connection boundary. */
export function applyOwnedFluentTheme(element: HTMLElement, dark: boolean | null): void {
  if (element !== element.ownerDocument.documentElement && !element.closest('body')) {
    appliedThemes.delete(element);
    return;
  }
  if (appliedThemes.has(element) && appliedThemes.get(element) === dark) return;
  setTheme(dark === null ? null : dark ? themes.dark : themes.light, element);
  for (const [role, value] of Object.entries(
    fluentThemeTokens(dark ? themes.dark : themes.light),
  )) {
    if (dark === null) element.style.removeProperty(`--sc-${role}`);
    else element.style.setProperty(`--sc-${role}`, value);
  }
  appliedThemes.set(element, dark);
}

/** Derive all brand roles through Microsoft's factories while retaining one visible accent. */
export function fluentAccentColorTokens(
  color: string,
  dark: boolean,
): Readonly<Record<string, string>> {
  const theme = (dark ? createDarkTheme : createLightTheme)(createAccentBrandPalette(color));
  const indicator = accentIndicatorColor(color, dark);
  const tokens: Record<string, string> = Object.fromEntries(
    Object.entries(theme).flatMap(([name, value]) =>
      /Brand|colorStrokeFocus2/.test(name) && typeof value === 'string' ? [[name, value]] : [],
    ),
  );
  for (const name of ['colorBrandBackground']) tokens[name] = color;
  for (const name of [
    'colorBrandForeground1',
    'colorCompoundBrandForeground1',
    'colorCompoundBrandBackground',
    'colorBrandStroke1',
    'colorCompoundBrandStroke',
    'colorStrokeFocus2',
  ])
    tokens[name] = indicator;
  tokens.colorBrandBackgroundHover = mixAccentColor(color, '#000000', 0.08);
  tokens.colorBrandBackgroundPressed = mixAccentColor(color, '#000000', 0.16);
  tokens.colorCompoundBrandBackgroundHover = mixAccentColor(indicator, '#000000', 0.08);
  tokens.colorCompoundBrandBackgroundPressed = mixAccentColor(indicator, '#000000', 0.16);
  tokens.colorNeutralForegroundOnBrand = accentForeground(color);
  return tokens;
}
