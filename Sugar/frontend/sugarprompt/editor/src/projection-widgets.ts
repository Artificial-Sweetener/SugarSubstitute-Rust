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
/** Literal native widgets share source-token identity without owning text, caret or history. */
import type { ReadonlyWidgetMedia } from './readonly-widget-media.js';
import { RangeValue } from '@codemirror/state';
import { WidgetType, type EditorView, type Decoration } from '@codemirror/view';
import { mountWeightInput, releaseWeightInput } from './weight-input-mount.js';
import { weightValue } from './weight-value.js';

/** Link prepared widgets to their one mapped native source range. */
export class SourceToken extends RangeValue {
  /** Whole-token pointer geometry can be reserved independently of concealed-marker exposure. */
  readonly exposesSyntaxOnHold: boolean;
  /** Prepare the intrinsic hold-rendering capability once with the canonical token. */
  constructor(exposesSyntaxOnHold = true) {
    super();
    this.exposesSyntaxOnHold = exposesSyntaxOnHold;
  }
}

/** Native source-backed widget; CodeMirror handles pointer selection and resource lifetime. */
export abstract class SourceWidget extends WidgetType {
  readonly token: SourceToken;
  /** Most concealed syntax reveals on interior selection; structural atomic rows use their dedicated name editor. */
  readonly revealOnInteriorSelection: boolean = true;
  /** Retain only the identity used by the projection's range index. */
  constructor(token: SourceToken) {
    super();
    this.token = token;
  }
  /** Canonical readonly mounts reuse the native renderer; media-bearing widgets defer only absolute pixels through the supplied finite owner. */
  toReadonlyDOM(view: EditorView, _media: ReadonlyWidgetMedia): HTMLElement {
    return this.toDOM(view);
  }
  /** Delegate pointer input to native selection across the widget's source boundaries. */
  override ignoreEvent(event: Event): boolean {
    return (
      event.target !== null &&
      event.target instanceof Element &&
      !!event.target.closest('.sp-exact-weight-input')
    );
  }
}

/** Independent wheel/adjustment bits derive solely from the existing prepared numeric intent. */
export type EmphasisFeedback = 0 | 1 | 2 | 3;

/** Font-relative emphasis chrome uses the exact authored weight spelling. */
export class EmphasisWidget extends SourceWidget {
  readonly part: 'prefix' | 'suffix';
  readonly weight: string;
  readonly synthetic: boolean;
  readonly draftKey: symbol | null;
  readonly feedback: EmphasisFeedback;
  /** Capture prepared literal display inputs outside rendering. */
  constructor(
    token: SourceToken,
    part: 'prefix' | 'suffix',
    weight = '',
    synthetic = false,
    draftKey: symbol | null = null,
    feedback: EmphasisFeedback = 0,
  ) {
    super(token);
    this.part = part;
    this.weight = weight;
    this.synthetic = synthetic;
    this.draftKey = draftKey;
    this.feedback = feedback;
  }
  /** Keep actual DOM when the semantic display inputs have not changed. */
  override eq(other: EmphasisWidget): boolean {
    return this.sameContent(other) && this.feedback === other.feedback;
  }
  /** Supported widget updates change only owned feedback classes, retaining exact input and glyph children. */
  override updateDOM(dom: HTMLElement, _view: EditorView, from: EmphasisWidget): boolean {
    if (!this.sameContent(from)) return false;
    this.paintFeedback(dom);
    return true;
  }
  /** Every semantic constructor input affecting the child graph participates in native reuse. */
  private sameContent(other: EmphasisWidget): boolean {
    return (
      this.part === other.part &&
      this.weight === other.weight &&
      this.synthetic === other.synthetic &&
      this.draftKey === other.draftKey
    );
  }
  /** Both parentheses accent; only the suffix numeric anchor publishes readiness and adjustment roles. */
  private paintFeedback(dom: HTMLElement): void {
    dom.classList.toggle('sp-emphasis-accent', this.feedback !== 0);
    dom.classList.toggle('sp-wheel-ready', this.part === 'suffix' && (this.feedback & 1) !== 0);
    dom.classList.toggle(
      'sp-adjustment-content',
      this.part === 'suffix' && (this.feedback & 2) !== 0,
    );
  }
  /** Construct emphasis chrome with literal DOM APIs. */
  override toDOM(view: EditorView): HTMLElement {
    const wrapper = document.createElement('span');
    wrapper.className = `sp-emphasis-${this.part}`;
    this.paintFeedback(wrapper);
    const parenthesis = document.createElement('span');
    parenthesis.className = 'sp-emphasis-parenthesis';
    parenthesis.textContent = this.part === 'prefix' ? '(' : ')';
    wrapper.append(parenthesis);
    if (this.part === 'suffix') {
      const weight = document.createElement('span');
      weight.className = 'sp-emphasis-weight';
      weight.dataset.weightNeutral = String(this.synthetic);
      if (this.draftKey) mountWeightInput(view, this.draftKey, weight, this.weight);
      else weight.append(weightValue(this.weight));
      wrapper.append(weight);
      wrapper.setAttribute('aria-label', `emphasis weight ${this.weight}`);
    }
    return wrapper;
  }
  /** CodeMirror widget removal releases its finite native numeric child. */
  override destroy(dom: HTMLElement): void {
    const cell = dom.querySelector<HTMLElement>('.sp-emphasis-weight');
    if (cell) releaseWeightInput(cell);
  }
}

/** A visible literal parenthesis retains its entire storage escape as one native caret step. */
export class ParenthesisWidget extends SourceWidget {
  readonly parenthesis: '(' | ')';
  /** Prepare the glyph independently of document mutation or DOM construction. */
  constructor(token: SourceToken, parenthesis: '(' | ')') {
    super(token);
    this.parenthesis = parenthesis;
  }
  /** Reuse actual DOM when the visible glyph remains the same. */
  override eq(other: ParenthesisWidget): boolean {
    return this.parenthesis === other.parenthesis;
  }
  /** Render one ordinary-font literal glyph; source remains owned by CodeMirror. */
  override toDOM(): HTMLElement {
    const glyph = document.createElement('span');
    glyph.className = 'sp-literal-parenthesis';
    glyph.textContent = this.parenthesis;
    return glyph;
  }
}

/** Whole semantic-role identity links concealed pairs and their literal body mark without another range or grammar owner. */
export function sourceDecorationToken(value: Decoration): SourceToken | null {
  const widget: unknown = value.spec.widget,
    token: unknown = value.spec.sourceToken;
  return widget instanceof SourceWidget
    ? widget.token
    : token instanceof SourceToken
      ? token
      : null;
}
