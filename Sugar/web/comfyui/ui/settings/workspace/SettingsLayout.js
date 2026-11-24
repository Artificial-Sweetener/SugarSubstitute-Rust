import { aboutPage } from './layout/AboutPageLayout.js';
import { generationPage } from './layout/GenerationPageLayout.js';
import { promptEditingPage } from './layout/PromptEditingPageLayout.js';
import { controlsPage } from './layout/ControlsPageLayout.js';
import { modelSourcesPage } from './layout/ModelSourcesPageLayout.js';
import { libraryPage } from './layout/LibraryPageLayout.js';
import { comfyuiPage } from './layout/ComfyUIPageLayout.js';
import { appearancePage } from './layout/AppearancePageLayout.js';
/** The workspace's pages in the Python application's navigation order. */
export const SETTINGS_LAYOUT = [
    aboutPage,
    generationPage,
    promptEditingPage,
    controlsPage,
    modelSourcesPage,
    libraryPage,
    comfyuiPage,
    appearancePage,
];
