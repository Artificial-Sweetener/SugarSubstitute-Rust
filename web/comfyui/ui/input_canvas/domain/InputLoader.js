/** Preserve known mask semantics and expose each image upload field as an independent source. */
export function recognizeInputLoaders(definition) {
    const { classType, channel } = definition;
    if (classType === 'SimpleSyrup.LoadMaskBatch' || classType === 'LoadImageMask')
        return [
            {
                kind: 'mask',
                classType,
                field: 'image',
                ordered: classType === 'SimpleSyrup.LoadMaskBatch',
                channel: 'channel',
            },
        ];
    if (classType === 'SimpleSyrup.LoadImageList' ||
        classType === 'LoadImage' ||
        classType === 'LoadImageOutput')
        return [
            {
                kind: 'image',
                classType,
                field: 'image',
                ordered: classType === 'SimpleSyrup.LoadImageList',
                channel: null,
            },
        ];
    const mask = definition.outputs.includes('MASK') && !definition.outputs.includes('IMAGE');
    const fields = definition.fields.filter((field) => field.upload &&
        (mask
            ? field.folder === null || field.folder === 'input'
            : field.folder === null || ['input', 'output', 'temp'].includes(field.folder)));
    if (mask && fields.length !== 1)
        return [];
    return fields.map((field) => ({
        kind: mask ? 'mask' : 'image',
        classType,
        field: field.name,
        ordered: field.ordered,
        channel: mask ? channel : null,
    }));
}
/** Keep source labels consistent across initial state, ordered controls and queue preparation. */
export function inputLabel(kind) {
    return kind === 'mask' ? 'Mask' : 'Image';
}
