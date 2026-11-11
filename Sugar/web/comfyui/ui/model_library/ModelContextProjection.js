import { createModelProviderIcon } from './vue/ModelProviderIcon.js';
/** Share identical navigation, icons and commands across field and node-title entry points. */
export function projectModelContext(items, command) {
    return items.map((item) => ({
        label: item.label,
        disabled: !item.enabled,
        ...(item.separator ? { separator: true } : {}),
        ...(item.checked === undefined ? {} : { checked: item.checked }),
        ...(item.children ? { children: projectModelContext(item.children, command) } : {}),
        ...(item.providerIcon
            ? { icon: (document) => createModelProviderIcon(document, item.providerIcon ?? '') }
            : {}),
        select: () => {
            if (item.url)
                window.open(item.url, '_blank', 'noopener,noreferrer');
            else
                command(item);
        },
    }));
}
