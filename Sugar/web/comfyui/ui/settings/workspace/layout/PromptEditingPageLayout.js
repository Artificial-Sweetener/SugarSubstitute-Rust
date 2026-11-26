import { row } from '../SettingsLayoutRows.js';
/** One feature switch's text and icon, keyed by its feature registry id. */
const featureRow = (section, id, title, description, icon) => row(`prompt_editing.${section}.${id}`, title, description, icon, undefined, [
    'prompt',
    'editor',
    'feature',
    id,
]);
/** The page's place in the navigation, its text, and its sections' rows. */
export const promptEditingPage = {
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
                row('prompt_editing.interaction.wheel_hover', 'Wheel adjust after hover', 'When off, click or focus a control before the mouse wheel can change it.', 'cursor-hover', undefined, ['wheel', 'hover', 'mouse', 'scroll', 'focus']),
            ],
        },
        {
            id: 'features',
            title: 'Editor features',
            rows: [
                featureRow('features', 'emphasis', 'Emphasis weights', 'Adjust weighted prompt text and show emphasis controls.', 'text-effects-sparkle'),
                featureRow('features', 'wildcard_syntax', 'Wildcard syntax', 'Highlight and resolve wildcard tokens.', 'braces-variable'),
                featureRow('features', 'wildcard_autocomplete', 'Wildcard autocomplete', 'Suggest wildcard completions while editing prompts.', 'text-bullet-list-square-sparkle'),
                featureRow('features', 'autocomplete_ghost_text', 'Autocomplete ghost text', 'Preview the selected autocomplete suffix inline while editing prompts.', 'text-field'),
                featureRow('features', 'lora_syntax', 'LoRA syntax', 'Parse and decorate Prompt Control LoRA schedule tokens in prompts.', 'tag-multiple'),
                featureRow('features', 'lora_autocomplete', 'LoRA autocomplete', 'Suggest LoRA names from the model catalog while editing prompts.', 'tag-search'),
                featureRow('features', 'lora_picker', 'LoRA picker', 'Insert Prompt Control LoRA schedule tokens from the prompt menu.', 'panel-right-cursor'),
                featureRow('features', 'lora_trigger_words', 'LoRA trigger words', 'Suggest trigger words for LoRAs that affect this prompt.', 'text-asterisk'),
                featureRow('features', 'segment_reorder', 'Segment reorder', 'Reorder prompt lines and comma-separated prompt segments.', 'reorder'),
                featureRow('features', 'spellcheck', 'Spellcheck', 'Underline prompt prose spelling issues and offer corrections.', 'text-grammar-checkmark'),
                featureRow('features', 'duplicate_segment_diagnostics', 'Duplicate segment warnings', 'Underline repeated prompt segments and offer cleanup actions.', 'text-bullet-list-square-warning'),
            ],
        },
        {
            id: 'autocomplete',
            title: 'Autocomplete',
            rows: [
                row('prompt_editing.autocomplete.manage_lists', 'Manage autocomplete lists', 'Add custom tags and hide unwanted tag suggestions.', 'folder-open', undefined, ['autocomplete', 'custom', 'censor', 'tags', 'suggestions']),
            ],
        },
        {
            id: 'wildcards',
            title: 'Wildcards',
            rows: [
                row('prompt_editing.wildcards.resolve', 'Resolve wildcards on generation', 'Expand wildcard prompt text before sending queued workflows to Comfy.', 'wand', undefined, ['wildcard', 'prompt', 'generation', 'resolve']),
                row('prompt_editing.wildcards.manage', 'Manage wildcards', 'Wildcards are .txt and .csv files in the sugar/wildcards folder of your ComfyUI user folder. A prompt names one as {colors}, {people/hair}, {csv:people:hair} or {colors|tag}, and each run picks from it with the run’s seed.', 'folder-open', undefined, ['wildcard', 'folder', 'refresh', 'prompt']),
            ],
        },
        {
            id: 'danbooru',
            title: 'Danbooru prompt integration',
            rows: [
                featureRow('danbooru', 'danbooru_url_import', 'Danbooru URL import', 'Paste supported Danbooru post or image URLs as prompt tags.', 'link-edit'),
            ],
        },
    ],
};
