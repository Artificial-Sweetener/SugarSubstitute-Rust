import { modelScopeMenu } from './ModelScopeMenus.js';
/** Attach Cube scope to the first eligible model field rather than an unrelated first mount. */
export function mountedModelContextContributions(mounts, root, app, snapshot, changed) {
    let scopeAdded = false;
    return [...mounts]
        .filter((mount) => mount.root === root)
        .map((mount) => {
        const scope = !scopeAdded && snapshot ? modelScopeMenu(app, mount.binding, snapshot, changed) : [];
        if (scope.length)
            scopeAdded = true;
        return { fieldName: mount.inputName, entries: [...mount.picker.contextEntries(), ...scope] };
    });
}
