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
/** Passive wrapper preparation owns the exact resolved declarations emitted by every canonical slot. */
import { requireValue } from '../../core/src/required.js';
import type { ReorderBaseContext } from './reorder-base-context.js';
/** Carrier suppression differs between whole clipped output and partial fragment output. */
export type PassiveWrapperPolicy = 'whole' | 'partial';
/** Final CSSOM declarations are both emitted paint and the exact wrapper reuse identity. */
export interface PreparedPassiveWrapperStyles {
  readonly policy: PassiveWrapperPolicy;
  readonly contentCss: string;
  readonly scrollerCss: string;
  readonly lineCss: string;
  readonly input: PassiveWrapperInput;
}
/** Complete CSSOM operands remain with the actual emitted plan for its finite slot lifetime. */
interface PassiveWrapperInput {
  readonly document: Document;
  readonly width: number;
  readonly content: readonly (readonly [string, string])[];
  readonly line: readonly (readonly [string, string])[];
  readonly contentCss: string;
  readonly scrollerCss: string;
  readonly lineCss: string;
}
/** Each prepared declaration scope belongs only to its matching canonical wrapper. */
type PassiveWrapperLayer = 'content' | 'scroller' | 'line';
/** Normalize one layer into the caller's detached declaration, retaining the exact native override sequence. */
function prepareWrapper(
  style: CSSStyleDeclaration,
  cssText: string,
  layer: PassiveWrapperLayer,
  context: Pick<ReorderBaseContext, 'width' | 'content' | 'line'>,
  policy: PassiveWrapperPolicy,
): string {
  style.cssText = cssText;
  if (layer === 'line') for (const [name, value] of context.line) style.setProperty(name, value);
  Object.assign(style, {
    display: 'block',
    ...(layer === 'scroller' ? { overflow: 'visible' } : {}),
    height: 'auto',
    width: `${context.width}px`,
    ...(layer !== 'scroller' ? { minHeight: '0' } : {}),
    margin: '0',
    ...(layer !== 'line' ? { padding: '0', border: '0' } : {}),
    transform: 'none',
    zoom: '1',
    ...(layer !== 'scroller' ? { visibility: policy === 'whole' ? 'visible' : 'hidden' } : {}),
  });
  if (layer === 'content')
    for (const [name, value] of context.content) style.setProperty(name, value);
  style.setProperty('transform', 'none', 'important');
  style.setProperty('zoom', '1', 'important');
  style.setProperty('width', `${context.width}px`, 'important');
  style.setProperty('box-sizing', 'border-box', 'important');
  return style.cssText;
}
/** WRITE reuses exact emitted output for identical operands, otherwise normalizing through one finite detached CSSOM scratch. */
export function preparePassiveWrapperStyles(
  document: Document,
  context: Pick<ReorderBaseContext, 'width' | 'content' | 'line'>,
  styles: {
    readonly contentCss: string;
    readonly scrollerCss: string;
    readonly lineCss: string;
  },
  policy: PassiveWrapperPolicy,
  previous?: PreparedPassiveWrapperStyles,
): PreparedPassiveWrapperStyles {
  const input = previous?.input;
  if (
    previous?.policy === policy &&
    input?.document === document &&
    input.width === context.width &&
    input.contentCss === styles.contentCss &&
    input.scrollerCss === styles.scrollerCss &&
    input.lineCss === styles.lineCss &&
    input.content.length === context.content.length &&
    input.content.every(
      ([name, value], index) =>
        context.content[index]?.[0] === name && context.content[index]?.[1] === value,
    ) &&
    input.line.length === context.line.length &&
    input.line.every(
      ([name, value], index) =>
        context.line[index]?.[0] === name && context.line[index]?.[1] === value,
    )
  )
    return requireValue(previous, 'reorder-passive-wrapper-style');
  const operands: PassiveWrapperInput = Object.freeze({
    document,
    width: context.width,
    content: Object.freeze(
      context.content.map(([name, value]) => Object.freeze([name, value] as const)),
    ),
    line: Object.freeze(context.line.map(([name, value]) => Object.freeze([name, value] as const))),
    contentCss: styles.contentCss,
    scrollerCss: styles.scrollerCss,
    lineCss: styles.lineCss,
  });
  const scratch = document.createElement('span');
  let outcome:
    | {
        readonly kind: 'success';
        readonly styles: PreparedPassiveWrapperStyles;
      }
    | {
        readonly kind: 'failure';
        readonly cause: unknown;
      };
  try {
    const style = scratch.style;
    outcome = {
      kind: 'success',
      styles: Object.freeze({
        policy,
        input: operands,
        contentCss: prepareWrapper(style, operands.contentCss, 'content', operands, policy),
        scrollerCss: prepareWrapper(style, operands.scrollerCss, 'scroller', operands, policy),
        lineCss: prepareWrapper(style, operands.lineCss, 'line', operands, policy),
      }),
    };
  } catch (cause) {
    outcome = { kind: 'failure', cause };
  }
  try {
    scratch.remove();
  } catch (cleanup) {
    if (outcome.kind === 'failure')
      throw new AggregateError(
        [outcome.cause, cleanup],
        'passive-wrapper-preparation-retirement-failed',
      );
    throw cleanup;
  }
  if (outcome.kind === 'failure') throw outcome.cause;
  return outcome.styles;
}
