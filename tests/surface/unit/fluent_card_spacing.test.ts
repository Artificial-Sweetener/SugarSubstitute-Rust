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
/** Prove graph cards retain native breathing room while masonry owns compact row spacing. */
import { afterEach, expect, test } from '@jest/globals';
import { fluentAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
});

/** Render native body spacing under the actual optional theme stylesheet. */
function fixture(masonry: boolean, fields: boolean, dark = true) {
  const nativeStyle = document.createElement('style');
  nativeStyle.textContent =
    '.native-body { padding:4px 0 12px; gap:4px; } .native-combo { height:24px; border:0; } .native-field-row,.native-header { border:0; padding:0; } .editable-text { padding:0 8px; } .native-collapse-slot { display:flex; padding:0 2px; }';
  const themeStyle = document.createElement('style');
  // Resolve border tokens at the JSDOM boundary, which cannot compute variable-based shorthands.
  themeStyle.textContent = fluentAppearanceCss(dark)
    .replaceAll('var(--sc-control-stroke)', 'rgba(255,255,255,.07)')
    .replaceAll('var(--sc-control-bottom-stroke)', 'rgba(255, 255, 255, 0.45)')
    .replaceAll('var(--sc-row-stroke)', dark ? 'rgba(0, 0, 0, 0.1)' : 'rgba(0, 0, 0, 0.06)');
  document.head.append(nativeStyle, themeStyle);
  const surface = document.createElement('div');
  if (masonry) surface.className = 'sugarcubes-workflow-masonry';
  const card = document.createElement('div');
  card.dataset.scFluentNode = '';
  const body = document.createElement('div');
  body.className = 'native-body';
  body.dataset.testid = 'node-body-test';
  const widgets = document.createElement('div');
  if (fields) widgets.dataset.scFluentFields = '';
  const control = document.createElement('input');
  control.value = 'custom value';
  widgets.append(control);
  body.append(widgets);
  card.append(body);
  surface.append(card);
  document.body.append(surface);
  return { card, widgets, body, control, themeStyle };
}

test('retains the native graph bottom padding and gap with themed widgets', () => {
  const { body, control, themeStyle } = fixture(false, true);
  expect(getComputedStyle(body).paddingBottom).toBe('12px');
  expect(getComputedStyle(body).paddingTop).toBe('4px');
  expect(getComputedStyle(body).gap).toBe('4px');
  expect(body.style.padding).toBe('');
  themeStyle.remove();
  expect(getComputedStyle(body).paddingBottom).toBe('12px');
  expect(control.value).toBe('custom value');
});

test('lets masonry field rows supply their own compact spacing', () => {
  const { body } = fixture(true, true);
  expect(getComputedStyle(body).paddingBottom).toBe('0px');
  expect(getComputedStyle(body).gap).toBe('0');
});

test('preserves custom bodies outside recognized native field layouts', () => {
  const { body, control } = fixture(true, false);
  expect(getComputedStyle(body).paddingBottom).toBe('12px');
  expect(control.value).toBe('custom value');
});

test.each([false, true])(
  'shares advanced-footer spacing in masonry=%s for open and closed options',
  (masonry) => {
    const { card, body, widgets, themeStyle } = fixture(masonry, true);
    const footer = document.createElement('button');
    footer.dataset.testid = 'advanced-inputs-button';
    card.append(footer);
    const advanced = document.createElement('div');
    advanced.dataset.scFluentRow = '';
    widgets.append(advanced);
    for (const expanded of [false, true]) {
      advanced.hidden = !expanded;
      footer.textContent = expanded ? 'Hide advanced inputs' : 'Show advanced inputs';
      expect(getComputedStyle(body).paddingBottom).toBe('0px');
      expect(getComputedStyle(body).paddingTop).toBe(masonry ? '0px' : '4px');
      expect(getComputedStyle(body).gap).toBe(masonry ? '0' : '4px');
    }
    themeStyle.remove();
    expect(getComputedStyle(body).paddingBottom).toBe('12px');
  },
);

test('retains native body spacing for custom cards with an advanced footer', () => {
  const { card, body } = fixture(false, false);
  const footer = document.createElement('button');
  footer.dataset.testid = 'advanced-inputs-button';
  card.append(footer);
  expect(getComputedStyle(body).paddingBottom).toBe('12px');
});

test('segments native graph ports from visible fields with a single reversible row seam', () => {
  const { body, widgets, themeStyle } = fixture(false, true);
  const ports = document.createElement('div');
  ports.dataset.scFluentPorts = '';
  const port = document.createElement('div');
  port.className = 'lg-slot';
  ports.append(port);
  body.prepend(ports);
  expect(getComputedStyle(ports).borderBottomWidth).toBe('1px');
  expect(getComputedStyle(ports).borderBottomColor).toBe('rgba(0, 0, 0, 0.1)');
  expect(getComputedStyle(ports).paddingTop).toBe('8px');
  expect(getComputedStyle(ports).paddingBottom).toBe('8px');
  expect(getComputedStyle(body).paddingTop).toBe('0px');
  expect(getComputedStyle(body).gap).toBe('0');
  expect(getComputedStyle(body).paddingBottom).toBe('12px');
  const field = widgets.firstElementChild;
  if (!(field instanceof HTMLElement)) throw new Error('Missing field fixture');
  field.hidden = true;
  ports.removeAttribute('data-sc-fluent-ports');
  expect(getComputedStyle(ports).borderBottomWidth).toBe('');
  field.hidden = false;
  ports.dataset.scFluentPorts = '';
  ports.hidden = true;
  expect(getComputedStyle(body).gap).toBe('4px');
  ports.hidden = false;
  port.remove();
  ports.removeAttribute('data-sc-fluent-ports');
  expect(getComputedStyle(ports).borderBottomWidth).toBe('');
  expect(getComputedStyle(body).gap).toBe('4px');
  ports.append(port);
  themeStyle.remove();
  expect(getComputedStyle(ports).borderBottomWidth).toBe('');
  expect(getComputedStyle(body).gap).toBe('4px');
});

test.each([false, true])(
  'matches combo and spinner frames in masonry=%s and restores native sizing',
  (masonry) => {
    const { body, themeStyle } = fixture(masonry, true);
    const combo = document.createElement('div');
    combo.className = 'native-combo';
    combo.dataset.scFluentComboFrame = '';
    combo.dataset.scFluentEditFrame = '';
    const trigger = document.createElement('button');
    trigger.dataset.testid = 'widget-select-default-trigger';
    trigger.setAttribute('role', 'combobox');
    trigger.setAttribute('aria-expanded', 'false');
    combo.append(trigger);
    const spinner = document.createElement('div');
    spinner.dataset.scNumberSpinner = '';
    body.append(combo, spinner);
    const comboStyle = getComputedStyle(combo);
    const spinnerStyle = getComputedStyle(spinner);
    expect(comboStyle.height).toBe('33px');
    expect(comboStyle.height).toBe(spinnerStyle.height);
    expect(comboStyle.boxSizing).toBe(spinnerStyle.boxSizing);
    expect(comboStyle.borderBottomColor).toBe(spinnerStyle.borderBottomColor);
    expect(comboStyle.borderBottomWidth).toBe(spinnerStyle.borderBottomWidth);
    expect(comboStyle.borderBottomWidth).toBe('1px');
    expect(comboStyle.borderBottomColor).toBe('rgba(255, 255, 255, 0.45)');
    expect(getComputedStyle(trigger).height).toBe('100%');
    // JSDOM does not compute pseudo-elements; prove the actual open-state selector owns the shared accent.
    const accentSelector =
      '[data-sc-fluent-edit-frame]:has(input:focus,textarea:focus,[role="combobox"]:focus-visible,[role="combobox"][aria-expanded="true"])';
    expect(combo.matches(accentSelector)).toBe(false);
    trigger.setAttribute('aria-expanded', 'true');
    expect(combo.matches(accentSelector)).toBe(true);
    expect(themeStyle.textContent).toContain(`${accentSelector}::after { transform:scaleX(1); }`);
    themeStyle.remove();
    expect(getComputedStyle(combo).height).toBe('24px');
  },
);

/** Mount only the native presentation boundaries owned by card segmentation. */
function segmentedFixture(masonry: boolean, dark = true) {
  const mounted = fixture(masonry, true, dark);
  const header = document.createElement('div');
  header.dataset.testid = 'node-header-test';
  header.className = 'native-header';
  mounted.card.prepend(header);
  const rows = [1, 2, 1].map((column, index) => {
    const row = document.createElement('div');
    row.className = 'native-field-row';
    row.dataset.scFluentRow = '';
    row.dataset.scFluentColumn = String(column);
    row.dataset.scFluentSpan = index === 2 ? '2' : '1';
    mounted.widgets.append(row);
    return row;
  });
  const first = rows[0];
  const second = rows[1];
  const third = rows[2];
  if (!first || !second || !third) throw new Error('Missing segmentation fixture rows');
  return { ...mounted, header, first, second, third };
}

test.each([false, true])('draws one header seam and row dividers in masonry=%s', (masonry) => {
  const { header, first, second, themeStyle } = segmentedFixture(masonry);
  expect(Number.parseFloat(getComputedStyle(header).borderBottomWidth)).toBe(masonry ? 0 : 1);
  expect(getComputedStyle(first).borderTopWidth).toBe(masonry ? '1px' : '0px');
  expect(getComputedStyle(second).borderTopWidth).toBe('1px');
  expect(getComputedStyle(second).borderTopColor).toBe('rgba(0, 0, 0, 0.1)');
  expect(getComputedStyle(second).paddingTop).toBe('8px');
  expect(getComputedStyle(second).paddingBottom).toBe('8px');
  expect(getComputedStyle(second).position).toBe(masonry ? 'relative' : '');
  themeStyle.remove();
  expect(getComputedStyle(header).borderBottomWidth).toBe('0px');
  expect(getComputedStyle(second).borderTopWidth).toBe('0px');
});

test.each([false, true])(
  'reserves Substitute trailing space once per scalar row in masonry=%s',
  (masonry) => {
    const { first, second, third, themeStyle } = segmentedFixture(masonry);
    expect(getComputedStyle(first).paddingRight).toBe(masonry ? '10px' : '30px');
    expect(getComputedStyle(second).paddingRight).toBe('30px');
    expect(getComputedStyle(third).paddingRight).toBe('30px');
    expect(getComputedStyle(third).paddingLeft).toBe('10px');
    third.dataset.scFluentKind = 'multiline';
    expect(getComputedStyle(third).paddingRight).toBe('10px');
    const custom = document.createElement('div');
    second.after(custom);
    expect(getComputedStyle(custom).paddingRight).toBe('');
    themeStyle.remove();
    expect(getComputedStyle(second).paddingRight).toBe('0px');
  },
);

test('centers a Substitute-sized chevron inside a larger click target at the header inset', () => {
  const { header } = segmentedFixture(true);
  header.dataset.scTitlebarControls = '';
  const content = document.createElement('div');
  const collapse = document.createElement('button');
  collapse.dataset.testid = 'node-collapse-button';
  const icon = document.createElement('i');
  collapse.append(icon);
  content.append(collapse);
  header.append(content);
  const buttonStyle = getComputedStyle(collapse);
  const iconStyle = getComputedStyle(icon);
  expect(buttonStyle.width).toBe('24px');
  expect(iconStyle.width).toBe('14px');
  expect(iconStyle.height).toBe('14px');
  const inset =
    Number.parseFloat(buttonStyle.right) +
    (Number.parseFloat(buttonStyle.width) - Number.parseFloat(iconStyle.width)) / 2;
  expect(inset).toBe(10);
  expect(getComputedStyle(content).paddingRight).toBe('20px');
  collapse.hidden = true;
  expect(getComputedStyle(collapse).display).toBe('none');
  expect(getComputedStyle(content).paddingRight).toBe('0px');
});

test.each([false, true])(
  'matches Substitute title and activation insets in masonry=%s',
  (masonry) => {
    const { header, themeStyle } = segmentedFixture(masonry);
    header.dataset.scTitlebarAction = 'activation';
    header.dataset.scTitlebarControls = '';
    const content = document.createElement('div');
    const titleGroup = document.createElement('div');
    const collapseSlot = document.createElement('div');
    collapseSlot.className = 'native-collapse-slot';
    const collapse = document.createElement('button');
    collapse.dataset.testid = 'node-collapse-button';
    collapse.hidden = true;
    collapseSlot.append(collapse);
    const title = document.createElement('div');
    title.dataset.testid = 'node-title';
    const editable = document.createElement('div');
    editable.className = 'editable-text';
    title.append(editable);
    const activation = document.createElement('label');
    activation.dataset.cubeCardActivation = 'mahiro_cfg';
    titleGroup.append(collapseSlot, title);
    content.append(titleGroup, activation);
    header.append(content);
    expect(getComputedStyle(collapseSlot).display).toBe('contents');
    const titleInset =
      Number.parseFloat(getComputedStyle(header).paddingLeft) +
      Number.parseFloat(getComputedStyle(title).paddingLeft || '0') +
      Number.parseFloat(getComputedStyle(editable).paddingLeft);
    expect(titleInset).toBe(masonry ? 40 : 10);
    expect(getComputedStyle(activation).paddingRight).toBe('8px');
    expect(getComputedStyle(content).paddingRight).toBe('0px');
    collapse.hidden = false;
    expect(getComputedStyle(content).paddingRight).toBe('20px');
    collapse.hidden = true;
    expect(getComputedStyle(content).paddingRight).toBe('0px');
    themeStyle.remove();
    expect(getComputedStyle(collapseSlot).display).toBe('flex');
    expect(getComputedStyle(editable).paddingLeft).toBe('8px');
    expect(getComputedStyle(activation).paddingRight).toBe('');
  },
);

test('omits separators for hidden leading fields and collapsed or widgetless headers', () => {
  const { header, first, second, third } = segmentedFixture(false);
  first.hidden = true;
  expect(getComputedStyle(second).borderTopWidth).toBe('0px');
  second.style.display = 'none';
  expect(getComputedStyle(third).borderTopWidth).toBe('0px');
  second.style.removeProperty('display');
  expect(getComputedStyle(third).borderTopWidth).toBe('1px');
  header.dataset.scTitlebarCollapsed = 'true';
  expect(getComputedStyle(header).borderBottomWidth).toBe('0px');
  delete header.dataset.scTitlebarCollapsed;
  for (const row of [first, second, third]) row.removeAttribute('data-sc-fluent-row');
  expect(getComputedStyle(header).borderBottomWidth).toBe('0px');
});

test('shares card stroke tint in both palettes and reserves short vertical dividers for masonry pairs', () => {
  const { second } = segmentedFixture(true, false);
  expect(getComputedStyle(second).borderTopColor).toBe('rgba(0, 0, 0, 0.06)');
  const css = fluentAppearanceCss(false);
  expect(css).toContain('--sc-row-stroke:var(--sc-card-stroke)');
  // Match the graph rule's capability predicates so masonry's single row seam wins the browser cascade.
  expect(css).toContain(
    '.sugarcubes-workflow-masonry [data-sc-fluent-node]:has([data-sc-fluent-row]) [data-testid^="node-header-"]:not([data-sc-titlebar-collapsed="true"]) { border-bottom:0; }',
  );
  // JSDOM omits pseudo-element geometry; browser verification covers the painted paired-field divider.
  expect(css).toContain(
    '.sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-column="2"]::before',
  );
  expect(css).toContain(
    'width:1px; height:33px; background:var(--sc-row-stroke); pointer-events:none;',
  );
});
