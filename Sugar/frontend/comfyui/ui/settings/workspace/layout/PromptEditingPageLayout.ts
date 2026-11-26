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
/** Prompt Editing: the prompt editor's behaviour, features, lists, wildcards and Danbooru paste. */
import type { FluentIconName } from '../../../core/FluentIcons.js';
import { row, type SettingsPageLayout, type SettingsRowLayout } from '../SettingsLayoutRows.js';

/** One feature switch's text and icon, keyed by its feature registry id. */
const featureRow = (
  section: 'features' | 'danbooru',
  id: string,
  title: string,
  description: string,
  icon: FluentIconName,
): SettingsRowLayout =>
  row(`prompt_editing.${section}.${id}`, title, description, icon, undefined, [
    'prompt',
    'editor',
    'feature',
    id,
  ]);

/** The page's place in the navigation, its text, and its sections' rows. */
export const promptEditingPage: SettingsPageLayout = {
  id: 'prompt_editing',
  title: 'Prompt Editing',
  subtitle: 'Prompt editor behavior and authoring support.',
  icon: 'text-effects-sparkle',
  searchable: true,
  sections: [
    {
      id: 'interaction',
      title: 'Interaction',
      rows: [
        row(
          'prompt_editing.interaction.wheel_hover',
          'Wheel adjust after hover',
          'When off, click or focus a control before the mouse wheel can change it.',
          'cursor-hover',
          undefined,
          ['wheel', 'hover', 'mouse', 'scroll', 'focus'],
        ),
      ],
    },
    {
      id: 'features',
      title: 'Editor features',
      rows: [
        featureRow(
          'features',
          'emphasis',
          'Emphasis weights',
          'Adjust weighted prompt text and show emphasis controls.',
          'text-effects-sparkle',
        ),
        featureRow(
          'features',
          'wildcard_syntax',
          'Wildcard syntax',
          'Highlight and resolve wildcard tokens.',
          'braces-variable',
        ),
        featureRow(
          'features',
          'wildcard_autocomplete',
          'Wildcard autocomplete',
          'Suggest wildcard completions while editing prompts.',
          'text-bullet-list-square-sparkle',
        ),
        featureRow(
          'features',
          'autocomplete_ghost_text',
          'Autocomplete ghost text',
          'Preview the selected autocomplete suffix inline while editing prompts.',
          'text-field',
        ),
        featureRow(
          'features',
          'lora_syntax',
          'LoRA syntax',
          'Parse and decorate Prompt Control LoRA schedule tokens in prompts.',
          'tag-multiple',
        ),
        featureRow(
          'features',
          'lora_autocomplete',
          'LoRA autocomplete',
          'Suggest LoRA names from the model catalog while editing prompts.',
          'tag-search',
        ),
        featureRow(
          'features',
          'lora_picker',
          'LoRA picker',
          'Insert Prompt Control LoRA schedule tokens from the prompt menu.',
          'panel-right-cursor',
        ),
        featureRow(
          'features',
          'lora_trigger_words',
          'LoRA trigger words',
          'Suggest trigger words for LoRAs that affect this prompt.',
          'text-asterisk',
        ),
        featureRow(
          'features',
          'segment_reorder',
          'Segment reorder',
          'Reorder prompt lines and comma-separated prompt segments.',
          'reorder',
        ),
        featureRow(
          'features',
          'spellcheck',
          'Spellcheck',
          'Underline prompt prose spelling issues and offer corrections.',
          'text-grammar-checkmark',
        ),
        featureRow(
          'features',
          'duplicate_segment_diagnostics',
          'Duplicate segment warnings',
          'Underline repeated prompt segments and offer cleanup actions.',
          'text-bullet-list-square-warning',
        ),
      ],
    },
    {
      id: 'autocomplete',
      title: 'Autocomplete',
      rows: [
        row(
          'prompt_editing.autocomplete.manage_lists',
          'Manage autocomplete lists',
          'Add custom tags and hide unwanted tag suggestions.',
          'folder-open',
          undefined,
          ['autocomplete', 'custom', 'censor', 'tags', 'suggestions'],
        ),
      ],
    },
    {
      id: 'wildcards',
      title: 'Wildcards',
      rows: [
        row(
          'prompt_editing.wildcards.resolve',
          'Resolve wildcards on generation',
          'Expand wildcard prompt text before sending queued workflows to Comfy.',
          'wand',
          undefined,
          ['wildcard', 'prompt', 'generation', 'resolve'],
        ),
        row(
          'prompt_editing.wildcards.manage',
          'Manage wildcards',
          'Wildcards are .txt and .csv files in the sugar/wildcards folder of your ComfyUI user folder. A prompt names one as {colors}, {people/hair}, {csv:people:hair} or {colors|tag}, and each run picks from it with the run’s seed.',
          'folder-open',
          undefined,
          ['wildcard', 'folder', 'refresh', 'prompt'],
        ),
      ],
    },
    {
      id: 'danbooru',
      title: 'Danbooru prompt integration',
      rows: [
        featureRow(
          'danbooru',
          'danbooru_url_import',
          'Danbooru URL import',
          'Paste supported Danbooru post or image URLs as prompt tags.',
          'link-edit',
        ),
      ],
    },
  ],
};
