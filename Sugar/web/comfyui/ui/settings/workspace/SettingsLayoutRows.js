/** A row's place and text, with only the fields it sets. */
export const row = (id, title, description, icon, wrap, keywords) => ({
    id,
    title,
    description,
    ...(icon ? { icon } : {}),
    ...(wrap ? { wrap } : {}),
    ...(keywords ? { keywords } : {}),
});
/** The workspace's pages in the Python application's navigation order. */
