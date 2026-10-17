import { SettingsSearchPlacement } from './SettingsSearchPlacement.js';
import { applyOwnedTheme } from '../surface/appearance/ThemeService.js';
import { SettingsPagePosition } from './SettingsPagePosition.js';
/** Own reversible settings annotations, descriptions and top-search placement for one dialog. */
export class SettingsWorkspaceSurface {
    root;
    catalog;
    #placement = null;
    #position = null;
    #rows = new Set();
    /** Bind an actual settings mount rather than discovering arbitrary dialogs or plugin DOM. */
    constructor(root, catalog) {
        this.root = root;
        this.catalog = catalog;
    }
    /** Apply Microsoft's official tokens to standard host settings and opt-in SugarCubes content. */
    update(fluent, dark) {
        this.root.setAttribute('data-sc-settings', '');
        if (fluent) {
            this.root.setAttribute('data-sc-settings-fluent', '');
            applyOwnedTheme(this.root, dark);
            this.#placement ??= new SettingsSearchPlacement(this.root);
            this.#position ??= new SettingsPagePosition(this.root);
            if (!this.#placement.reconcile())
                this.#position.reconcile();
        }
        else {
            this.root.removeAttribute('data-sc-settings-fluent');
            applyOwnedTheme(this.root, null);
            this.#placement?.dispose();
            this.#placement = null;
            this.#position?.dispose();
            this.#position = null;
        }
        for (const row of this.#rows) {
            if (row.isConnected)
                continue;
            this.#clearRow(row);
            this.#rows.delete(row);
        }
        for (const row of this.root.querySelectorAll('[data-setting-id]')) {
            const id = row.dataset.settingId ?? '';
            const descriptor = this.catalog.find(id);
            if (!descriptor)
                continue;
            if (descriptor.kind === 'custom' && !id.startsWith('SugarCubes.'))
                continue;
            this.#rows.add(row);
            row.dataset.scSettingKind = descriptor.kind === 'standard' ? 'standard' : 'owned';
            if (!fluent || !descriptor.description) {
                row.querySelector('.sc-setting-description')?.remove();
                continue;
            }
            const label = row.querySelector('.form-label');
            if (!label || label.querySelector('.sc-setting-description'))
                continue;
            const description = this.root.ownerDocument.createElement('small');
            description.className = 'sc-setting-description sc-settings-owned';
            description.textContent = descriptor.description;
            label.append(description);
        }
    }
    /** Leave host markup and custom-page lifetimes exactly where Comfy expects them. */
    dispose() {
        this.#placement?.dispose();
        this.#placement = null;
        this.#position?.dispose();
        this.#position = null;
        for (const row of this.#rows)
            this.#clearRow(row);
        this.#rows.clear();
        applyOwnedTheme(this.root, null);
        this.root.removeAttribute('data-sc-settings');
        this.root.removeAttribute('data-sc-settings-fluent');
    }
    /** Remove only presentation owned by this adapter. */
    #clearRow(row) {
        row.removeAttribute('data-sc-setting-kind');
        row.querySelector('.sc-setting-description')?.remove();
    }
}
