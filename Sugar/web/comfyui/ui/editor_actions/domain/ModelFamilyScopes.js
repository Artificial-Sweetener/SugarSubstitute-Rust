const aliases = [
    ['anima', 'Anima', /\banima\b/i],
    ['sdxl', 'SDXL', /\b(?:sdxl|sdxl\s*1\.?0|stable\s+diffusion\s+xl)\b/i],
    ['illustrious', 'Illustrious', /\billustrious\b/i],
    ['noobai', 'NoobAI', /\bnoob\s*ai\b|\bnoobai\b/i],
    ['pony', 'Pony', /\bpony\b/i],
    ['flux', 'FLUX', /\bflux\b/i],
];
/** Retain detected architecture alongside Substitute-compatible family associations. */
export function modelFamilyScopes(evidence) {
    if (!evidence)
        return [];
    const base = aliases.find(([, , pattern]) => pattern.test(evidence.baseModel));
    const families = [...(base ? [base] : [])];
    if (!base || base[0] === 'sdxl')
        for (const text of [evidence.title, evidence.tags.join(' ')]) {
            const family = aliases.find(([, , pattern]) => pattern.test(text));
            if (family && !families.some(([key]) => key === family[0]))
                families.push(family);
        }
    const scopes = families.map(([key, label]) => ({
        scope: 'model_family',
        provider: 'civitai',
        key,
        label,
    }));
    if (evidence.architecture) {
        const alias = aliases.find(([, label]) => label === evidence.architecture);
        const architecture = {
            scope: 'model_family',
            provider: alias ? 'civitai' : 'comfy',
            key: alias?.[0] ?? evidence.architecture.toLowerCase(),
            label: evidence.architecture,
        };
        if (!scopes.some((scope) => scope.provider === architecture.provider && scope.key === architecture.key))
            scopes.unshift(architecture);
    }
    return scopes;
}
