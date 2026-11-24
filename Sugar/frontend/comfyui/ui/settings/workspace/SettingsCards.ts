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
 * Draw settings rows as the Python application's setting cards: icon, title and description
 * on the left, the controls on the right (moving under the text below the row's wrap width),
 * switch rows toggled by a click anywhere on the card, switch expanders that show their rows
 * while on, and chevron expanders. Controls follow their values live.
 */
import { createFluentIcon } from '../../core/FluentIcons.js';
import { NumericSpinnerSlider } from '../../surface/appearance/NumericSpinnerSlider.js';
import type { SettingsControl, SettingsIcon, SettingsRow, SettingsValue } from './SettingsModel.js';

/** Python's default wrap width: below it a card's controls move under its text. */
export const DEFAULT_WRAP = 476;

/** A drawn card and the subscriptions that keep it live. */
export interface SettingsCardView {
  readonly element: HTMLElement;
  dispose(): void;
}

/** Draw one row. `report` receives write failures; the control then shows the stored value. */
export function renderSettingsRow(
  document: Document,
  row: SettingsRow,
  report: (error: unknown) => void,
  nested = false,
): SettingsCardView {
  const stops: (() => void)[] = [];
  const card = document.createElement('div');
  card.className = nested ? 'sc-sw-row' : 'sc-sw-card';
  card.dataset.row = row.id;
  card.dataset.wrap = String(row.wrap ?? DEFAULT_WRAP);
  if (row.body) {
    card.classList.add('sc-sw-body-card');
    // One page body that cannot be drawn is reported; the rest of the workspace still shows.
    try {
      card.append(row.body(document));
    } catch (error: unknown) {
      report(error);
    }
    return { element: card, dispose: () => undefined };
  }
  const head = nested ? card : document.createElement('div');
  if (!nested) {
    head.className = 'sc-sw-card-head';
    card.append(head);
  }
  if (row.icon && !nested) head.append(icon(document, row.icon));
  const text = document.createElement('div');
  text.className = 'sc-sw-card-text';
  const title = document.createElement('div');
  title.className = 'sc-sw-card-title';
  title.textContent = row.title;
  text.append(title);
  if (row.description) {
    const description = document.createElement('div');
    description.className = 'sc-sw-card-description';
    description.textContent = row.description;
    text.append(description);
  }
  head.append(text);
  const controls = document.createElement('div');
  controls.className = 'sc-sw-card-controls';
  head.append(controls);
  const controlElements: HTMLElement[] = [];
  // Controls that draw their own disabled state follow the row's enabled state through these.
  const enabledFollowers: (() => void)[] = [];
  const context: ControlContext = {
    label: row.title,
    enabled: () => row.enabled?.read() ?? true,
    followEnabled: (follow) => enabledFollowers.push(follow),
  };
  const headerSwitch = row.expander?.switch;
  for (const control of [
    ...(headerSwitch ? [{ kind: 'switch', value: headerSwitch } as const] : []),
    ...(row.controls ?? []),
  ]) {
    const drawn = renderControl(document, context, control, report, stops);
    controlElements.push(drawn);
    controls.append(drawn);
  }
  if (!controlElements.length) controls.hidden = true;
  // A switch row is one target: a click anywhere on its card flips the switch.
  const onlySwitch = controlElements.length === 1 && controlElements[0]?.matches('[role="switch"]');
  if (onlySwitch) {
    card.classList.add('sc-sw-interactive');
    head.addEventListener('click', (event) => {
      const target = event.target;
      if (target instanceof Element && target.closest('.sc-sw-card-controls')) return;
      controlElements[0]?.click();
    });
  }
  if (row.enabled) {
    const enabled = row.enabled;
    const apply = (): void => {
      const on = enabled.read();
      card.toggleAttribute('data-disabled', !on);
      for (const element of controls.querySelectorAll<HTMLElement>('button,input,select'))
        if (!element.closest('[data-sc-number-pair]'))
          element.toggleAttribute('disabled', !on || element.hasAttribute('data-sc-sw-disabled'));
      for (const follow of enabledFollowers) follow();
    };
    apply();
    stops.push(enabled.subscribe(apply));
  }
  if (row.visible) {
    const visible = row.visible;
    const apply = (): void => {
      card.hidden = !visible.read();
    };
    apply();
    stops.push(visible.subscribe(apply));
  }
  if (row.expander) {
    const expander = row.expander;
    card.classList.add('sc-sw-expander');
    const body = document.createElement('div');
    body.className = 'sc-sw-card-body';
    for (const child of expander.rows) {
      const drawn = renderSettingsRow(document, child, report, true);
      stops.push(drawn.dispose);
      body.append(drawn.element);
    }
    card.append(body);
    if (expander.switch) {
      const open = expander.switch;
      const apply = (): void => {
        card.toggleAttribute('data-open', open.read());
      };
      apply();
      stops.push(open.subscribe(apply));
    } else {
      card.classList.add('sc-sw-interactive');
      const chevron = document.createElement('span');
      chevron.className = 'sc-sw-chevron';
      chevron.title = 'Show all settings';
      chevron.append(createFluentIcon(document, 'chevron-down', 14));
      head.append(chevron);
      head.addEventListener('click', (event) => {
        const target = event.target;
        if (target instanceof Element && target.closest('.sc-sw-card-controls')) return;
        card.toggleAttribute('data-open');
      });
    }
  }
  return {
    element: card,
    dispose: () => {
      for (const stop of stops.splice(0)) stop();
    },
  };
}

function icon(document: Document, value: SettingsIcon): HTMLElement {
  const slot = document.createElement('span');
  slot.className = 'sc-sw-card-icon';
  slot.setAttribute('aria-hidden', 'true');
  slot.append(
    typeof value === 'function' ? value(document) : createFluentIcon(document, value, 20),
  );
  return slot;
}

/** Write a value; a failure is reported and the control returns to the stored value. */
function write<T>(
  value: SettingsValue<T>,
  next: T,
  report: (error: unknown) => void,
  refresh: () => void,
): void {
  try {
    const result = value.write(next);
    if (result instanceof Promise)
      result.catch((error: unknown) => {
        report(error);
        refresh();
      });
  } catch (error: unknown) {
    report(error);
    refresh();
  }
}

/** What a control knows of its row. */
interface ControlContext {
  readonly label: string;
  enabled(): boolean;
  followEnabled(follow: () => void): void;
}

function renderControl(
  document: Document,
  context: ControlContext,
  control: SettingsControl,
  report: (error: unknown) => void,
  stops: (() => void)[],
): HTMLElement {
  switch (control.kind) {
    case 'switch': {
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'sc-sw-switch';
      button.setAttribute('role', 'switch');
      const label = document.createElement('span');
      const track = document.createElement('span');
      track.className = 'sc-sw-switch-track';
      button.append(label, track);
      const show = (): void => {
        const on = control.value.read();
        button.setAttribute('aria-checked', String(on));
        label.textContent = on ? 'On' : 'Off';
      };
      show();
      stops.push(control.value.subscribe(show));
      button.addEventListener('click', () => {
        const next = button.getAttribute('aria-checked') !== 'true';
        button.setAttribute('aria-checked', String(next));
        label.textContent = next ? 'On' : 'Off';
        write(control.value, next, report, show);
      });
      return button;
    }
    case 'choice': {
      const select = document.createElement('select');
      select.className = 'sc-sw-field sc-sw-combo';
      select.style.width = `${String(control.width ?? 180)}px`;
      const options = (): void => {
        const choices = control.choices.read();
        const ids = choices.map((choice) => choice.value).join('\n');
        if (select.dataset.choices !== ids) {
          select.dataset.choices = ids;
          select.replaceChildren(
            ...choices.map((choice) => {
              const option = document.createElement('option');
              option.value = choice.value;
              option.textContent = choice.label;
              return option;
            }),
          );
        }
        select.value = control.value.read();
      };
      options();
      stops.push(control.choices.subscribe(options), control.value.subscribe(options));
      select.addEventListener('change', () => write(control.value, select.value, report, options));
      return select;
    }
    case 'number':
    case 'slider': {
      // The node cards' number control: their spinbox, with the style's slider when bounded.
      const pair = new NumericSpinnerSlider(
        document,
        {
          name: context.label,
          label: context.label,
          precision: null,
          minimum: control.min ?? Number.NEGATIVE_INFINITY,
          maximum: control.max ?? Number.POSITIVE_INFINITY,
          step: control.step ?? 1,
        },
        control.kind === 'slider',
        (next) => write(control.value, next, report, show),
      );
      const show = (): void => pair.show(control.value.read(), !context.enabled());
      show();
      context.followEnabled(show);
      stops.push(control.value.subscribe(show), () => pair.dispose());
      return pair.root;
    }
    case 'text': {
      const input = document.createElement('input');
      input.type = 'text';
      input.className = 'sc-sw-field';
      input.style.width = `${String(control.width ?? 240)}px`;
      if (control.placeholder) input.placeholder = control.placeholder;
      if (control.readOnly) {
        input.readOnly = true;
        input.dataset.scSwReadonly = '';
      }
      const show = (): void => {
        if (input !== document.activeElement || control.readOnly)
          input.value = control.value.read();
      };
      show();
      stops.push(control.value.subscribe(show));
      // Python saves a field when editing finishes: Enter or leaving the field.
      input.addEventListener('change', () => write(control.value, input.value, report, show));
      input.addEventListener('keydown', (event) => {
        if (event.key === 'Enter') input.blur();
        if (event.key === 'Escape') {
          // Abandon the edit: the stored value returns before leaving, so nothing is saved.
          input.value = control.value.read();
          input.blur();
        }
      });
      return input;
    }
    case 'button': {
      const button = document.createElement('button');
      button.type = 'button';
      button.className = control.button.primary ? 'sc-sw-button sc-sw-primary' : 'sc-sw-button';
      button.textContent = control.button.label;
      let busy = false;
      const enabled = control.button.enabled;
      const apply = (): void => {
        const off = busy || (enabled ? !enabled.read() : false);
        button.toggleAttribute('data-sc-sw-disabled', off);
        if (!button.closest('[data-disabled]')) button.disabled = off;
      };
      apply();
      if (enabled) stops.push(enabled.subscribe(apply));
      button.addEventListener('click', (event) => {
        event.stopPropagation();
        if (busy) return;
        busy = true;
        apply();
        const done = (): void => {
          busy = false;
          apply();
        };
        // The action starts within the click, so its own guards take effect before any other input.
        try {
          Promise.resolve(control.button.run()).catch(report).finally(done);
        } catch (error: unknown) {
          report(error);
          done();
        }
      });
      return button;
    }
    case 'status': {
      const status = document.createElement('span');
      status.className = 'sc-sw-status';
      const show = (): void => {
        status.textContent = control.text.read();
      };
      show();
      stops.push(control.text.subscribe(show));
      return status;
    }
    case 'element': {
      const element = control.create(document);
      if (control.dispose) stops.push(() => control.dispose?.());
      return element;
    }
  }
}
