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
/** Verify scoped, idempotent Cube pack labels in native picker results. */

import { ComfyCubePickerResultPresenter } from '../../../frontend/comfyui/ui/picker/ComfyCubePickerResultPresenter.js';
import type { ComfyCubeNodeDefinition } from '../../../frontend/comfyui/ui/picker/ComfyCubeNodeDefProjector.js';

test('shows pack provenance instead of the structural Cube category detail', () => {
  document.body.replaceChildren(
    resultList(resultItem('Anima/Prompt by Region', 'SugarCubes / Anima')),
  );
  const presenter = new ComfyCubePickerResultPresenter({
    document,
    definitions: () => [definition()],
  });

  presenter.refresh();

  const category = document.querySelector<HTMLElement>('[data-sugarcubes-picker-result-type]');
  expect(category?.textContent).toBe('Base-Cubes');
  expect(category?.getAttribute('data-sugarcubes-picker-result-type')).toBe(definition().name);
  const title = document.querySelector<HTMLElement>('[data-sugarcubes-picker-model-title]');
  expect(title?.querySelector('.sugarcubes-model-title')?.getAttribute('aria-label')).toBe(
    'Anima/Prompt by Region',
  );
  expect(title?.querySelector('[data-sugarcubes-model-pill]')?.textContent).toBe('Anima');
  expect(title?.textContent).toBe('AnimaPrompt by Region');
});

test('does not mutate an already-correct picker detail during reconciliation', async () => {
  document.body.replaceChildren(
    resultList(resultItem('Anima/Prompt by Region', 'SugarCubes / Anima')),
  );
  const presenter = new ComfyCubePickerResultPresenter({
    document,
    definitions: () => [definition()],
  });
  presenter.refresh();
  const result = document.querySelector<HTMLElement>('[data-testid="result-item"]');
  if (!result) throw new Error('Expected presented Cube result');
  let mutations = 0;
  const observer = new MutationObserver((records) => {
    mutations += records.length;
  });
  observer.observe(result, { childList: true, characterData: true, subtree: true });

  presenter.refresh();
  await Promise.resolve();

  observer.disconnect();
  expect(mutations).toBe(0);
});

test('leaves ordinary and ambiguous native results untouched', () => {
  const ordinary = resultItem('KSampler', 'sampling');
  const ambiguous = resultItem('Shared Cube', 'SugarCubes / SDXL', 'Same description');
  document.body.replaceChildren(resultList(ordinary, ambiguous));
  const first = definition({
    name: `SugarCubes.Cube.${'b'.repeat(64)}`,
    display_name: 'Shared Cube',
    category: 'SugarCubes/SDXL',
    description: 'Same description',
  });
  const second = {
    ...first,
    name: `SugarCubes.Cube.${'c'.repeat(64)}`,
    sugarcubes_pack_name: 'Other',
  };
  const presenter = new ComfyCubePickerResultPresenter({
    document,
    definitions: () => [first, second],
  });

  presenter.refresh();

  expect(ordinary.textContent).toContain('sampling');
  expect(ambiguous.textContent).toContain('SugarCubes / SDXL');
  expect(document.querySelector('[data-sugarcubes-picker-result-type]')).toBeNull();
  expect(document.querySelector('[data-sugarcubes-picker-model-title]')).toBeNull();
});

test('renders markup-like picker titles literally and leaves nonmatching model routes plain', () => {
  const literal = resultItem('<Anima>/<script>alert(1)</script>', 'SugarCubes / <Anima>');
  const nonmatching = resultItem('Flux/Detailer', 'SugarCubes / SDXL', 'Other description');
  document.body.replaceChildren(resultList(literal, nonmatching));
  const presenter = new ComfyCubePickerResultPresenter({
    document,
    definitions: () => [
      definition({
        display_name: '<Anima>/<script>alert(1)</script>',
        category: 'SugarCubes/<Anima>',
        sugarcubes_target_model: '<Anima>',
      }),
      definition({
        name: `SugarCubes.Cube.${'d'.repeat(64)}`,
        display_name: 'Flux/Detailer',
        description: 'Other description',
        category: 'SugarCubes/SDXL',
        sugarcubes_target_model: 'SDXL',
      }),
    ],
  });

  presenter.refresh();

  expect(literal.querySelector('[data-sugarcubes-model-pill]')?.textContent).toBe('<Anima>');
  expect(literal.querySelector('script')).toBeNull();
  expect(nonmatching.querySelector('[data-sugarcubes-picker-model-title]')).toBeNull();
  expect(nonmatching.textContent).toContain('Flux/Detailer');
});

/** Build a projected definition with overridable result identity. */
function definition(overrides: Partial<ComfyCubeNodeDefinition> = {}): ComfyCubeNodeDefinition {
  return {
    name: `SugarCubes.Cube.${'a'.repeat(64)}`,
    display_name: 'Anima/Prompt by Region',
    description: 'Regional Prompting',
    category: 'SugarCubes/Anima',
    python_module: 'custom_nodes.Base-Cubes',
    sugarcubes_pack_name: 'Base-Cubes',
    sugarcubes_target_model: 'Anima',
    output_node: false,
    input: { required: {} },
    input_order: { required: [] },
    output: [],
    output_name: [],
    output_is_list: [],
    search_aliases: [],
    ...overrides,
  };
}

/** Build Comfy's stable result-list host around test options. */
function resultList(...results: HTMLElement[]): HTMLElement {
  const list = document.createElement('div');
  list.id = 'results-list';
  list.append(...results);
  return list;
}

/** Build the relevant semantic shape of Comfy's native result component. */
function resultItem(
  name: string,
  category: string,
  description = 'Regional Prompting',
): HTMLElement {
  const result = document.createElement('div');
  result.setAttribute('data-testid', 'result-item');
  const container = document.createElement('div');
  container.className = 'option-container';
  const content = document.createElement('div');
  const titleRow = document.createElement('div');
  titleRow.className = 'text-sm';
  const title = document.createElement('span');
  title.className = 'truncate';
  title.textContent = name;
  titleRow.append(title);
  const detailRow = document.createElement('div');
  const categoryLabel = document.createElement('span');
  categoryLabel.textContent = category;
  const descriptionLabel = document.createElement('span');
  descriptionLabel.textContent = description;
  detailRow.append(categoryLabel, descriptionLabel);
  content.append(titleRow, detailRow);
  container.append(content);
  result.append(container);
  return result;
}
