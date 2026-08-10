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

/** Present aggregate default choices with the complete implementation diff on demand. */

import { $el } from '/scripts/ui.js';
import { ModalShell } from './ModalShell.js';
import type { ModalAdapter } from './ModalShell.js';
import type {
  CubeDefaultDecision,
  CubeDefaultDecisions,
  CubeDefaultReview,
  CubeImplementationChange,
} from '../save/CubeDefaultReviewService.js';

/** Own the concise Comfy-styled implementation-save decision surface. */
export class CubeDefaultChangesModal {
  readonly #shell: ModalShell;

  /** Bind the established SugarCubes modal shell. */
  constructor({ adapter }: { adapter?: ModalAdapter | null } = {}) {
    this.#shell = new ModalShell({
      adapter: adapter ?? null,
      variantClassName: 'sugarcubes-default-review-overlay',
      dialogClassName: 'sugarcubes-default-review-dialog',
    });
  }

  /** Resolve aggregate default choices, or null when the author cancels. */
  open(reviews: readonly CubeDefaultReview[]): Promise<CubeDefaultDecisions | null> {
    const decisions = new Map<string, CubeDefaultDecision>();
    const body = $el('div.sugarcubes-default-review');
    const multiple = reviews.length > 1;

    for (const review of reviews) {
      const decision: CubeDefaultDecision = {
        overwriteDefaults: false,
        savePromptFields: false,
      };
      decisions.set(review.cubeId, decision);
      body.append(this.#buildDecisionSection(review, decision, multiple));
    }
    body.append(buildMoreInfo(reviews));

    const name = reviews[0]?.displayName ?? 'cube';
    return this.#shell.open({
      title: multiple ? `Save changes to ${reviews.length} cubes?` : `Save changes to ${name}?`,
      description:
        'The implementation will be updated. Choose whether this save should also replace its current defaults.',
      body,
      confirmLabel: 'Save implementation',
      cancelLabel: 'Cancel',
      cancelResult: null,
      allowOverlayClose: false,
      onConfirm: () => this.#shell.close(toDecisions(decisions)),
      initialFocus: () => this.#shell.elements.confirmButton,
    }) as Promise<CubeDefaultDecisions | null>;
  }

  /** Render only the two aggregate decisions that apply to one cube. */
  #buildDecisionSection(
    review: CubeDefaultReview,
    decision: CubeDefaultDecision,
    showName: boolean,
  ): HTMLElement {
    const section = $el('section.sugarcubes-default-review__cube');
    if (showName) {
      section.append($el('h3', { textContent: review.displayName }));
    }
    const choices = $el('div.sugarcubes-default-review__choices');
    if (review.overwriteDefaultCount > 0) {
      choices.append(
        createChoice({
          label: 'Overwrite current defaults',
          description: `Uses the current values for ${formatCount(review.overwriteDefaultCount, 'existing control default')}.`,
          onChange: (checked) => {
            decision.overwriteDefaults = checked;
          },
        }),
      );
    }
    if (review.promptDefaultCount > 0) {
      choices.append(
        createChoice({
          label: 'Save prompt fields as cube defaults',
          description: `Includes ${formatCount(review.promptDefaultCount, 'prompt field')}.`,
          onChange: (checked) => {
            decision.savePromptFields = checked;
          },
        }),
      );
    }
    section.append(choices);
    return section;
  }
}

/** Build one unchecked aggregate choice. */
function createChoice({
  label,
  description,
  onChange,
}: {
  label: string;
  description: string;
  onChange(checked: boolean): void;
}): HTMLLabelElement {
  const input = $el('input', { type: 'checkbox' }) as HTMLInputElement;
  input.addEventListener('change', () => onChange(input.checked));
  return $el('label.sugarcubes-default-review__choice', [
    input,
    $el('span', [
      $el('strong', { textContent: label }),
      $el('small', { textContent: description }),
    ]),
  ]) as HTMLLabelElement;
}

/** Build the separate read-only implementation diff disclosure. */
function buildMoreInfo(reviews: readonly CubeDefaultReview[]): HTMLElement {
  const container = $el('div.sugarcubes-default-review__more');
  const button = $el('button.p-button.p-component.p-button-text', {
    type: 'button',
    textContent: 'More info',
    'aria-expanded': 'false',
  }) as HTMLButtonElement;
  const details = $el('div.sugarcubes-default-review__details');
  details.hidden = true;
  for (const review of reviews) {
    details.append(buildChangeList(review, reviews.length > 1));
  }
  button.addEventListener('click', () => {
    details.hidden = !details.hidden;
    button.setAttribute('aria-expanded', details.hidden ? 'false' : 'true');
    button.textContent = details.hidden ? 'More info' : 'Hide details';
  });
  container.append(button, details);
  return container;
}

/** Group one cube's complete possible commit diff by product-facing section. */
function buildChangeList(review: CubeDefaultReview, showName: boolean): HTMLElement {
  const container = $el('section.sugarcubes-default-review__change-list');
  if (showName) container.append($el('h3', { textContent: review.displayName }));
  const bySection = new Map<string, CubeImplementationChange[]>();
  for (const change of review.changes) {
    const changes = bySection.get(change.section) ?? [];
    changes.push(change);
    bySection.set(change.section, changes);
  }
  for (const [section, changes] of bySection) {
    const group = $el('section.sugarcubes-default-review__change-group');
    group.append($el('h4', { textContent: section }));
    for (const change of changes) group.append(buildChangeRow(change));
    container.append(group);
  }
  return container;
}

/** Render one literal, non-interactive implementation change. */
function buildChangeRow(change: CubeImplementationChange): HTMLElement {
  const previous = change.previousExists ? formatValue(change.previousValue) : 'Not present';
  const proposed = change.proposedExists ? formatValue(change.proposedValue) : 'Removed';
  const annotation = decisionAnnotation(change);
  return $el('div.sugarcubes-default-review__change', [
    $el('strong', { textContent: change.label }),
    $el('span', { textContent: `${previous} → ${proposed}` }),
    ...(annotation ? [$el('small', { textContent: annotation })] : []),
  ]);
}

/** Explain which aggregate choice controls a conditional change. */
function decisionAnnotation(change: CubeImplementationChange): string {
  if (change.decision === 'overwrite_defaults')
    return 'When “Overwrite current defaults” is selected';
  if (change.decision === 'save_prompt_fields') {
    return 'When “Save prompt fields as cube defaults” is selected';
  }
  return '';
}

function toDecisions(decisions: ReadonlyMap<string, CubeDefaultDecision>): CubeDefaultDecisions {
  return Object.fromEntries(decisions);
}

/** Format one count with correct singular or plural copy. */
function formatCount(count: number, singular: string): string {
  return `${count} ${singular}${count === 1 ? '' : 's'}`;
}

/** Format dynamic values as complete literal text inside the opt-in detail view. */
function formatValue(value: unknown): string {
  if (typeof value === 'string') return value || 'Empty';
  const encoded = JSON.stringify(value);
  return encoded ?? String(value);
}
