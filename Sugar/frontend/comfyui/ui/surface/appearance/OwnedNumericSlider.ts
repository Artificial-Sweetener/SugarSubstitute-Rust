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
 * The one slider for number values on surfaces SugarCubes owns (settings, canvas tools): the
 * slider the node cards show in the selected visual style. The Comfy style installs no node-card
 * styles, so there it is ComfyUI's own slider, as ComfyUI's settings draw one.
 */
import { createNumericSlider } from './ControlFactory.js';
import {
  CONTROL_HEIGHT,
  NUMERIC_PAIR_GAP,
  NUMERIC_SLIDER_WIDTH,
  SPINNER_WIDTH,
} from './ControlGeometry.js';
import type { NumericSliderControl } from './NativeNumericControls.js';
import type { NumericSliderSpec } from './NumericSliderPresentation.js';
import { readDocumentVisualStyle, subscribeVisualStyle } from './VisualStyleBoundary.js';

const STYLE_ID = 'sugarcubes-owned-numeric-controls';
const COMFY = 'html:is(:not([data-sc-visual-style]),[data-sc-visual-style="comfy"])';

/** Give an owned control's container the selected style's tokens and field materials. */
export function ownStyledNumericSurface(element: HTMLElement): void {
  element.setAttribute('data-sc-style-theme-root', '');
  element.setAttribute('data-sc-style-surface', '');
  ensureOwnedNumericStyles(element.ownerDocument);
}

/**
 * The node cards' slider in the selected style, or ComfyUI's slider in the Comfy style, replaced
 * in place when the visual style changes. `input` receives each value a drag passes; the slider
 * element carries the spec's `attributes` across replacements.
 */
export class StyledNumericSlider {
  #slider: NumericSliderControl;
  #state: [number, number, number, boolean, number | null] | null = null;
  readonly #stopStyle: () => void;

  constructor(
    private readonly document: Document,
    private readonly spec: NumericSliderSpec & {
      readonly attributes?: Readonly<Record<string, string>>;
    },
    private readonly input: (value: number) => void,
  ) {
    ensureOwnedNumericStyles(document);
    this.#slider = this.#create();
    this.#stopStyle = subscribeVisualStyle(document, () => {
      const previous = this.#slider;
      this.#slider = this.#create();
      previous.root.replaceWith(this.#slider.root);
      if (this.#state) this.#slider.synchronize(...this.#state);
    });
  }

  /** The current slider element; a style change replaces it where it stands. */
  get root(): HTMLElement {
    return this.#slider.root;
  }

  synchronize(
    minimum: number,
    maximum: number,
    step: number,
    disabled: boolean,
    value: number | null,
  ): void {
    this.#state = [minimum, maximum, step, disabled, value];
    this.#slider.synchronize(minimum, maximum, step, disabled, value);
  }

  /** Stop following the visual style. */
  dispose(): void {
    this.#stopStyle();
  }

  #create(): NumericSliderControl {
    const slider =
      readDocumentVisualStyle(this.document) === 'comfy'
        ? createComfySlider(this.document)
        : createNumericSlider(this.document, this.spec);
    const root = slider.root;
    root.setAttribute('data-sc-number-slider', '');
    root.setAttribute('aria-label', `${this.spec.label} slider`);
    // The selected style's tokens, wherever the slider is placed.
    root.setAttribute('data-sc-style-theme-root', '');
    root.setAttribute('data-sc-style-surface', '');
    for (const [name, value] of Object.entries(this.spec.attributes ?? {}))
      root.setAttribute(name, value);
    const follow = (): void => {
      const value = slider.readValue();
      if (Number.isFinite(value)) this.input(value);
    };
    root.addEventListener('input', follow);
    root.addEventListener('change', follow);
    return slider;
  }
}

function createComfySlider(document: Document): NumericSliderControl {
  const input = document.createElement('input');
  input.type = 'range';
  input.className = 'sc-comfy-numeric-slider';
  return {
    root: input,
    readValue: () => input.valueAsNumber,
    synchronize: (minimum, maximum, step, disabled, value) => {
      input.min = String(minimum);
      input.max = String(maximum);
      input.step = String(step);
      input.disabled = disabled;
      if (value !== null) input.value = String(value);
    },
  };
}

/**
 * The owned pair's geometry, and ComfyUI's number widget and slider for the Comfy style. Every
 * other style draws owned controls with its node-card rules through their `data-sc-number-*`
 * markers.
 */
function ensureOwnedNumericStyles(document: Document): void {
  if (document.getElementById(STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = STYLE_ID;
  const pair = '.sc-numeric-spinner-slider';
  style.textContent = `
    ${pair} { display:flex; align-items:center; gap:${NUMERIC_PAIR_GAP}; height:${CONTROL_HEIGHT}; }
    ${pair} > [data-sc-number-spinner] { flex:none; }
    ${pair} > [data-sc-number-slider] { width:${NUMERIC_SLIDER_WIDTH}; flex:none; }
    .sc-comfy-numeric-slider { margin:0; accent-color:var(--p-primary-color,var(--ring,#df258e)); }
    ${COMFY} ${pair} [data-sc-number-spinner] { display:flex; align-items:stretch; width:${SPINNER_WIDTH}; height:100%;
      box-sizing:border-box; border:1px solid var(--border-color,#4e4e4e); border-radius:6px;
      background:var(--comfy-input-bg,#222); color:var(--input-text,var(--fg-color,#ddd)); overflow:hidden; }
    ${COMFY} ${pair} [data-sc-number-spinner] > div { flex:1; min-width:0; }
    ${COMFY} ${pair} [data-sc-number-input] { width:100%; height:100%; box-sizing:border-box; padding:0 4px; border:0;
      background:transparent; color:inherit; font:inherit; text-align:center; outline:none; }
    ${COMFY} ${pair} :is([data-sc-number-increment],[data-sc-number-decrement]) { flex:none; width:24px; padding:0; border:0;
      background:transparent; color:inherit; display:grid; place-items:center; cursor:pointer; }
    ${COMFY} ${pair} :is([data-sc-number-increment],[data-sc-number-decrement]):hover:not(:disabled) {
      background:color-mix(in srgb,currentColor 10%,transparent); }
    ${COMFY} ${pair} :is([data-sc-number-increment],[data-sc-number-decrement]):disabled { opacity:.4; cursor:default; }`;
  document.head.append(style);
}
