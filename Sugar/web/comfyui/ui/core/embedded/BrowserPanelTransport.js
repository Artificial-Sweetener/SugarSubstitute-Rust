/** Identify one navigation without accepting duplicate or partially matched identities. */
export function panelNavigation(search) {
    const query = new URLSearchParams(search);
    const host = query.get('sc-panel-host');
    const session = query.get('sc-panel-session');
    if (!host ||
        !session ||
        !/^[a-zA-Z0-9._-]{1,128}(?![\s\S])/.test(host) ||
        !/^[a-zA-Z0-9-]{16,80}(?![\s\S])/.test(session) ||
        query.getAll('sc-panel-host').length !== 1 ||
        query.getAll('sc-panel-session').length !== 1)
        return null;
    return { host, session };
}
