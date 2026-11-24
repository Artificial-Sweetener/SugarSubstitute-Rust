/** The page's place in the navigation, its text, and its sections' rows. */
export const aboutPage = {
    id: 'about',
    title: 'About',
    subtitle: 'Version, project, and acknowledgements',
    icon: 'cube-filled',
    searchable: false,
    sections: [
        {
            id: 'about',
            title: '',
            rows: [
                { id: 'about.page', title: 'About' },
                { id: 'about.updates', title: 'Updates' },
            ],
        },
    ],
};
