/** Serialize trusted provider values under the product's own CSS namespace. */
export function themeTokenDeclarations(tokens) {
    return Object.entries(tokens)
        .map(([role, value]) => `--sc-${role}:${value};`)
        .join('');
}
