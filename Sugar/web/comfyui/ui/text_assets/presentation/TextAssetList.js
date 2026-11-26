/** Draw `assets` into `list`, grouped in the order their groups first appear. */
export function renderTextAssetList(list, assets, selected, emptyText, events) {
    const document = list.ownerDocument;
    if (!assets.length) {
        const empty = document.createElement('p');
        empty.className = 'sc-text-assets__empty';
        empty.textContent = emptyText;
        list.replaceChildren(empty);
        return;
    }
    const groups = new Map();
    for (const asset of assets)
        groups.set(asset.group, [...(groups.get(asset.group) ?? []), asset]);
    const rows = [];
    // Rows alternate across the whole list, as they did in SugarSubstitute.
    let stripe = 0;
    for (const [group, members] of groups) {
        const heading = document.createElement('div');
        heading.className = 'sc-text-assets__group';
        heading.textContent = `${group} (${String(members.length)})`;
        rows.push(heading);
        for (const asset of members) {
            const row = document.createElement('button');
            row.type = 'button';
            row.className = 'sc-text-assets__row';
            row.setAttribute('role', 'option');
            row.setAttribute('aria-selected', String(asset.id === selected));
            row.dataset.asset = asset.id;
            if (stripe++ % 2 === 0)
                row.classList.add('is-alternate');
            if (asset.enabled === false)
                row.classList.add('is-disabled');
            row.title = asset.subtitle;
            const label = document.createElement('span');
            label.className = 'sc-text-assets__label';
            label.textContent = asset.label;
            const subtitle = document.createElement('span');
            subtitle.className = 'sc-text-assets__subtitle';
            subtitle.textContent = asset.subtitle;
            row.append(label, subtitle);
            row.addEventListener('click', () => events.select(asset));
            row.addEventListener('contextmenu', (event) => {
                event.preventDefault();
                events.select(asset);
                events.menu(asset, event.clientX, event.clientY);
            });
            rows.push(row);
        }
    }
    list.replaceChildren(...rows);
}
