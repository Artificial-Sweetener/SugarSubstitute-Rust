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
/** Prove every Fluent field control inherits one field-surface primitive for its state fills. */
import { afterEach, describe, expect, test } from '@jest/globals';
import { webDarkTheme, webLightTheme } from '@fluentui/tokens';
import { fluentAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';
import { fluentThemeTokens } from '../../../frontend/comfyui/ui/surface/appearance/FluentThemeTokens.js';

type FieldState = 'rest' | 'hover' | 'focus' | 'disabled';

/** Build one instance of each field kind inside a themed node card. */
function mountFields(): Record<string, { host: HTMLElement; focusTarget: HTMLElement }> {
  const card = document.createElement('section');
  card.setAttribute('data-sc-style-node', '');
  const combo = document.createElement('div');
  combo.setAttribute('data-sc-style-edit-frame', '');
  combo.setAttribute('data-sc-style-combo-frame', '');
  const trigger = document.createElement('button');
  trigger.setAttribute('role', 'combobox');
  trigger.setAttribute('aria-expanded', 'false');
  combo.append(trigger);
  const spinner = document.createElement('div');
  spinner.setAttribute('data-sc-style-edit-frame', '');
  spinner.setAttribute('data-sc-number-spinner', '');
  const numberInput = document.createElement('input');
  numberInput.setAttribute('data-sc-number-input', '');
  spinner.append(numberInput);
  const textFrame = document.createElement('div');
  textFrame.setAttribute('data-sc-style-edit-frame', '');
  const text = document.createElement('input');
  text.type = 'text';
  textFrame.append(text);
  const multilineFrame = document.createElement('div');
  multilineFrame.setAttribute('data-sc-style-edit-frame', '');
  const multiline = document.createElement('textarea');
  multilineFrame.append(multiline);
  const prompt = document.createElement('div');
  prompt.setAttribute('data-sc-editor-field', 'fluent');
  const editor = document.createElement('div');
  editor.tabIndex = 0;
  prompt.append(editor);
  const model = document.createElement('div');
  model.className = 'sc-model-field';
  const modelInput = document.createElement('input');
  model.append(modelInput);
  card.append(combo, spinner, textFrame, multilineFrame, prompt, model);
  document.body.append(card);
  return {
    combo: { host: combo, focusTarget: trigger },
    number: { host: spinner, focusTarget: numberInput },
    text: { host: text, focusTarget: text },
    multiline: { host: multiline, focusTarget: multiline },
    prompt: { host: prompt, focusTarget: editor },
    model: { host: model, focusTarget: modelInput },
  };
}

/** Flatten the installed stylesheet's style rules, including reduced-motion media blocks. */
function styleRules(sheet: CSSStyleSheet): CSSStyleRule[] {
  return Array.from(sheet.cssRules).flatMap((rule) =>
    rule instanceof CSSStyleRule
      ? [rule]
      : rule instanceof CSSMediaRule
        ? Array.from(rule.cssRules).filter((entry) => entry instanceof CSSStyleRule)
        : [],
  );
}

/** Match a selector, expressing :focus-within through :has(:focus), which JSDOM does evaluate. */
function matchesRule(element: HTMLElement, rule: CSSStyleRule): boolean {
  const selector = rule.selectorText;
  // JSDOM's selector engine never matches :focus-within; the substitutes are its exact definition.
  return (
    element.matches(selector) ||
    (selector.includes(':focus-within') &&
      (element.matches(selector.replaceAll(':focus-within', ':focus')) ||
        element.matches(selector.replaceAll(':focus-within', ':has(:focus)'))))
  );
}

/** Resolve which declared state fill wins for an element, honoring source order of equal rules. */
function resolvedSurface(rules: readonly CSSStyleRule[], element: HTMLElement): string {
  const matching = rules.filter(
    (rule) => rule.style.getPropertyValue('--sc-field-surface') && matchesRule(element, rule),
  );
  return matching.at(-1)?.style.getPropertyValue('--sc-field-surface').trim() ?? '';
}

/** Install the Fluent stylesheet for one host color scheme. */
function installFluent(dark: boolean): CSSStyleRule[] {
  const style = document.createElement('style');
  style.textContent = fluentAppearanceCss(dark);
  document.head.append(style);
  if (!style.sheet) throw new Error('The Fluent stylesheet did not parse.');
  return styleRules(style.sheet);
}

afterEach(() => {
  document.head.replaceChildren();
  document.body.replaceChildren();
});

test.each([
  [true, 'rgba(255,255,255,0.0605)', 'rgba(255,255,255,0.0837)', 'rgba(30,30,30,0.7)'],
  [false, 'rgba(255,255,255,0.7)', 'rgba(249,249,249,0.5)', '#ffffff'],
])('dark=%s supplies the qfluentwidgets input fill ramp', (dark, rest, hover, focus) => {
  const tokens = fluentThemeTokens(dark ? webDarkTheme : webLightTheme, dark);
  expect(tokens['field-fill']).toBe(rest);
  expect(tokens['field-hover-fill']).toBe(hover);
  expect(tokens['field-focus-fill']).toBe(focus);
});

describe.each([true, false])('Fluent dark=%s', (dark) => {
  test('paints every field kind from the one resolved surface and nowhere else', () => {
    const rules = installFluent(dark);
    for (const [kind, { host }] of Object.entries(mountFields())) {
      const painters = rules.filter(
        (rule) =>
          (rule.style.getPropertyValue('background-color') ||
            rule.style.getPropertyValue('background')) &&
          host.matches(rule.selectorText),
      );
      if (kind === 'prompt') {
        expect(painters).toEqual([]);
        const delegate = rules.filter(
          (rule) =>
            rule.style.getPropertyValue('--background-color') && host.matches(rule.selectorText),
        );
        expect(delegate.map((rule) => rule.style.getPropertyValue('--background-color'))).toEqual([
          'var(--sc-field-surface, var(--sc-field-fill))',
        ]);
        continue;
      }
      expect({
        kind,
        painters: painters.map((rule) => [
          rule.selectorText,
          rule.style.getPropertyValue('background-color'),
        ]),
      }).toEqual({
        kind,
        painters: [
          [expect.stringContaining(':is([data-sc-style-combo-frame], '), 'var(--sc-field-surface)'],
        ],
      });
    }
  });

  test('resolves rest, hover, focus and disabled through the same token for every field kind', () => {
    const rules = installFluent(dark);
    const stateRules = rules.filter((rule) => rule.style.getPropertyValue('--sc-field-surface'));
    // One rule per state: a control-specific state rule would be an ad hoc fill owner.
    expect(stateRules).toHaveLength(4);
    const fields = mountFields();
    const expected: Record<FieldState, string> = {
      rest: 'var(--sc-field-fill)',
      hover: 'var(--sc-field-hover-fill)',
      focus: 'var(--sc-field-focus-fill)',
      disabled: 'var(--sc-field-disabled-fill)',
    };
    for (const [kind, { host, focusTarget }] of Object.entries(fields)) {
      expect({ kind, rest: resolvedSurface(rules, host) }).toEqual({ kind, rest: expected.rest });
      focusTarget.focus();
      if (kind === 'combo') {
        // A focused, closed combo remains a button; its search session is the editing state.
        expect(resolvedSurface(rules, host)).toBe(expected.rest);
        focusTarget.setAttribute('aria-expanded', 'true');
      }
      expect({ kind, focus: resolvedSurface(rules, host) }).toEqual({
        kind,
        focus: expected.focus,
      });
      focusTarget.blur();
      focusTarget.setAttribute('aria-expanded', 'false');
      if (focusTarget instanceof HTMLInputElement || focusTarget instanceof HTMLTextAreaElement)
        focusTarget.disabled = true;
      else focusTarget.setAttribute('aria-disabled', 'true');
      if (kind !== 'prompt')
        expect({ kind, disabled: resolvedSurface(rules, host) }).toEqual({
          kind,
          disabled: expected.disabled,
        });
    }
  });
});
