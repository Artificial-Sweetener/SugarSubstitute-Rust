const fluentIcons = {
    fixed: 'locked',
    randomize: 'game-die',
    increment: 'add',
    decrement: 'subtract',
    'reuse-last': 'reuse-last',
};
const nativeIcons = {
    fixed: 'pencil-off',
    randomize: 'shuffle',
    'reuse-last': 'rotate-ccw',
};
/** Resolve the pinned Microsoft asset used by Fluent seed controls. */
export function seedIconImage(action) {
    const url = new URL(`../../../../icons/seed/${fluentIcons[action]}.svg`, import.meta.url);
    return `url("${url.href}")`;
}
/** Keep icon colors inherited while matching Comfy's native +1/-1 generation glyphs. */
export function createSeedActionIcon(document, action, fluent) {
    const icon = document.createElement('span');
    icon.dataset.scSeedIcon = '';
    icon.dataset.scSeedIconFamily = fluent ? 'fluent' : 'comfy';
    icon.style.cssText =
        'width:16px;height:16px;display:inline-flex;align-items:center;justify-content:center;flex:none;';
    if (fluent) {
        icon.style.background = 'currentColor';
        icon.style.mask = `${seedIconImage(action)} center/contain no-repeat`;
        icon.style.setProperty('--sc-seed-icon', seedIconImage(action));
    }
    else if (action === 'increment' || action === 'decrement') {
        icon.textContent = action === 'increment' ? '+1' : '-1';
    }
    else
        icon.className = `icon-[lucide--${nativeIcons[action]}]`;
    return icon;
}
