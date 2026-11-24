/** The pages, in the Python app's navigation order; pages with no settings today are left out. */
export const SETTINGS_PAGES = [
    {
        id: 'about',
        title: 'About',
        subtitle: 'Version, project, updates and acknowledgements.',
        icon: 'app-generic',
        sections: ['About', 'Updates'],
    },
    {
        id: 'generation',
        title: 'Generation',
        subtitle: 'Generation behavior and generated files.',
        icon: 'play',
        sections: ['Output canvas', 'Output files', 'Model updates', 'Memory'],
    },
    {
        id: 'prompt_editing',
        title: 'Prompt Editing',
        subtitle: 'Prompt editor behavior and authoring support.',
        icon: 'edit',
        sections: ['Wildcards'],
    },
    {
        id: 'controls',
        title: 'Controls',
        subtitle: 'Keyboard and pointer controls.',
        icon: 'settings',
        sections: ['Workflow connections'],
    },
    {
        id: 'model_sources',
        title: 'Model Sources',
        subtitle: 'External providers, metadata, previews and caches.',
        icon: 'brain-circuit',
        sections: ['Model library', 'Model metadata', 'Model previews'],
    },
    {
        id: 'library',
        title: 'Library',
        subtitle: 'Cube Packs, their updates and authoring.',
        icon: 'folder',
        sections: ['Cube library', 'Authoring'],
    },
    {
        id: 'appearance',
        title: 'Appearance',
        subtitle: 'Visual customization.',
        icon: 'color',
        sections: ['Style', 'Cube stack'],
    },
];
/** Where each SugarCubes setting family belongs: its section label. */
export const sugarCubesSettingsSections = {
    about: 'About',
    updates: 'Updates',
    canvas: 'Output canvas',
    output: 'Output files',
    modelUpdates: 'Model updates',
    memory: 'Memory',
    prompts: 'Wildcards',
    connections: 'Workflow connections',
    models: 'Model library',
    metadata: 'Model metadata',
    previews: 'Model previews',
    packs: 'Cube library',
    authoring: 'Authoring',
    appearance: 'Style',
    stack: 'Cube stack',
};
/** The section order across all pages, first to last. */
const SECTION_ORDER = SETTINGS_PAGES.flatMap((page) => page.sections);
/**
 * Place one setting: under the SugarCubes category, in its section. ComfyUI shows higher
 * `sortOrder` first, so the Python order counts down.
 */
export function sugarCubesSettingsSection(section, key) {
    const label = sugarCubesSettingsSections[section];
    return {
        category: ['SugarCubes', label, key],
        sortOrder: 10_000 - SECTION_ORDER.indexOf(label) * 10,
    };
}
/** The page a section belongs to. */
export function settingsPageOfSection(label) {
    return SETTINGS_PAGES.find((page) => page.sections.includes(label)) ?? null;
}
