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
/** Own one card's activation presentation while the card-state service owns node enablement. */
import type { ComfyNode } from '../types/graph.js';
import type { CubeFaceCardDecision } from './CubeFaceCardPolicy.js';
import { createActivationSwitch } from './appearance/ControlFactory.js';
import type { ActivationSwitchControl } from './appearance/ActivationSwitchControl.js';
import { readOwnedVisualStyle, subscribeVisualStyle } from './appearance/VisualStyleBoundary.js';

/** Keep a stable header accessory across value updates and visual-style transitions. */
export class CubeFaceActivationControl {
  readonly root: HTMLDivElement;
  readonly #onChange: (node: ComfyNode, enabled: boolean) => void;
  readonly #releaseStyle: () => void;
  #card: CubeFaceCardDecision;
  #switch: ActivationSwitchControl;
  #style;

  /** Create presentation without mutating node state until the user activates the control. */
  constructor(
    document: Document,
    card: CubeFaceCardDecision,
    onChange: (node: ComfyNode, enabled: boolean) => void,
  ) {
    this.#card = card;
    this.#onChange = onChange;
    this.root = document.createElement('div');
    this.root.className = 'sugarcubes-cube-face__activation';
    this.root.dataset.cubeCardActivation = card.id;
    this.#style = readOwnedVisualStyle(document);
    this.#switch = createActivationSwitch(document);
    this.#mountSwitch();
    this.root.addEventListener('pointerdown', this.#stopPropagation);
    this.root.addEventListener('click', this.#stopPropagation);
    this.#releaseStyle = subscribeVisualStyle(document, () => this.#refreshStyle());
    this.update(card);
  }

  /** Synchronize external state without replacing the focused control or emitting an edit. */
  update(card: CubeFaceCardDecision): void {
    this.#card = card;
    this.root.dataset.enabled = String(card.enabled);
    const actionLabel = `${card.enabled ? 'Disable' : 'Enable'} ${card.label}`;
    this.root.title = actionLabel;
    this.#switch.synchronize(card.enabled, actionLabel, card.enabled ? 'Enabled' : 'Disabled');
  }

  /** Release presentation subscriptions when the native card mount retires. */
  dispose(): void {
    this.#releaseStyle();
    this.#switch.control.removeEventListener('change', this.#change);
    this.root.removeEventListener('pointerdown', this.#stopPropagation);
    this.root.removeEventListener('click', this.#stopPropagation);
  }

  /** Retain the accessory and node state when the selected provider changes. */
  #refreshStyle(): void {
    const style = readOwnedVisualStyle(this.root.ownerDocument);
    if (style === this.#style) return;
    const control = this.#switch.control;
    const focused =
      this.root.ownerDocument.activeElement === control ||
      this.root.contains(this.root.ownerDocument.activeElement);
    control.removeEventListener('change', this.#change);
    this.#style = style;
    this.#switch = createActivationSwitch(this.root.ownerDocument);
    this.#mountSwitch();
    this.update(this.#card);
    if (focused) this.#switch.control.focus();
  }

  /** Connect exactly one edit callback to the active provider. */
  #mountSwitch(): void {
    this.#switch.control.addEventListener('change', this.#change);
    this.root.replaceChildren(this.#switch.root);
  }

  /** Forward user edits to the authoritative card-state service exactly once. */
  readonly #change = (): void => {
    const enabled = this.#switch.control.checked;
    this.update({ ...this.#card, enabled });
    this.#onChange(this.#card.node, enabled);
  };

  /** Keep activation gestures from also invoking titlebar or canvas actions. */
  readonly #stopPropagation = (event: Event): void => event.stopPropagation();
}
