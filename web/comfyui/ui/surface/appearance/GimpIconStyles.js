const cardIcons = {
    application: 'system-run-symbolic.svg',
    edit: 'document-edit-symbolic.svg',
    eraser: 'edit-clear-all-symbolic.svg',
    folder: 'folder-open-symbolic.svg',
    model: 'document-open-symbolic.svg',
    palette: 'color-select-symbolic.svg',
    photo: 'image-x-generic-symbolic.svg',
};
const actionIcons = [
    { file: 'document-edit-symbolic.svg', aliases: ['icon-[lucide--pencil]', 'pi-pencil'] },
    { file: 'document-open-symbolic.svg', aliases: ['icon-[lucide--folder-open]', 'pi-folder-open'] },
    { file: 'folder-new-symbolic.svg', aliases: ['icon-[lucide--folder-plus]', 'pi-folder-plus'] },
    { file: 'document-save-symbolic.svg', aliases: ['icon-[lucide--save]', 'pi-save'] },
    {
        file: 'preferences-system-symbolic.svg',
        aliases: ['icon-[lucide--settings]', 'icon-[lucide--settings-2]', 'pi-cog'],
    },
    {
        file: 'edit-undo-symbolic.svg',
        aliases: ['icon-[lucide--undo]', 'icon-[lucide--undo-2]', 'pi-undo'],
    },
    {
        file: 'edit-redo-symbolic.svg',
        aliases: ['icon-[lucide--redo]', 'icon-[lucide--redo-2]', 'pi-redo'],
    },
    {
        file: 'view-refresh-symbolic.svg',
        aliases: ['icon-[lucide--refresh-cw]', 'icon-[lucide--refresh-ccw]', 'pi-refresh'],
    },
    { file: 'window-close-symbolic.svg', aliases: ['icon-[lucide--x]', 'pi-times'] },
    { file: 'zoom-in-symbolic.svg', aliases: ['icon-[lucide--zoom-in]', 'pi-search-plus'] },
    { file: 'zoom-out-symbolic.svg', aliases: ['icon-[lucide--zoom-out]', 'pi-search-minus'] },
    { file: 'zoom-fit-best-symbolic.svg', aliases: ['icon-[lucide--scan]', 'pi-expand'] },
];
/** Resolve pinned assets relative to their deployed presentation module. */
function iconUrl(file) {
    return `url("${new URL(`../../../../icons/node/gimp/${file}`, import.meta.url).href}")`;
}
/** Keep every swap conditional on GIMP so changing theme restores the original artwork. */
export function gimpIconCss() {
    const root = 'html[data-sc-visual-style="gimp"]';
    const surfaces = ':is([data-sc-theme="gimp"],[data-sc-style-node],[data-sc-style-surface],[data-sc-style-popup],[data-sc-menu-surface],[data-sc-plugin-toolbar],[data-sc-substitute-toolbar])';
    const actions = `${root} :is(${surfaces} :is(button,[data-sc-button],[role^="menuitem"]),[data-sugarcubes-save-cube-icon])`;
    const cards = Object.entries(cardIcons)
        .map(([role, file]) => `${root} .sugarcubes-workflow-masonry__node-icon[data-masonry-node-icon="${role}"]::before { mask-image:${iconUrl(file)}; }`)
        .join('\n');
    const controls = actionIcons
        .map(({ file, aliases }) => {
        const selector = `${actions} :is(${aliases.map((alias) => `[class~="${alias}"]`).join(',')})`;
        return `${selector} { background:currentColor; mask-image:${iconUrl(file)}; mask-position:center; mask-size:contain; mask-repeat:no-repeat; }
      ${selector}::before { visibility:hidden; }`;
    })
        .join('\n');
    return `${root} .sugarcubes-cube-face__node-card [data-masonry-title-icon] { padding-left:22px; }
    ${root} .sugarcubes-workflow-masonry__node-icon { width:16px; height:16px; }
    ${root} .sugarcubes-workflow-masonry__node-icon::before { width:16px; height:16px; margin:0; }
    ${cards}
    ${controls}
    ${root} :is([data-sc-plugin-toolbar],[data-sc-substitute-toolbar]) .sc-application-gear { mask-image:${iconUrl('preferences-system-symbolic.svg')}; }`;
}
