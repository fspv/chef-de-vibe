import { createContext, useContext } from 'react';
export const DEFAULT_CONTEXT_VALUE = {
    hunkClassName: '',
    lineClassName: '',
    gutterClassName: '',
    codeClassName: '',
    monotonous: false,
    gutterType: 'default',
    viewType: 'split',
    widgets: {},
    hideGutter: false,
    selectedChanges: [],
    generateAnchorID: () => undefined,
    generateLineClassName: () => undefined,
    renderGutter: ({ renderDefault, wrapInAnchor }) => wrapInAnchor(renderDefault()),
    codeEvents: {},
    gutterEvents: {},
};
const ContextType = createContext(DEFAULT_CONTEXT_VALUE);
export const Provider = ContextType.Provider;
export const useDiffSettings = () => useContext(ContextType);
//# sourceMappingURL=index.js.map