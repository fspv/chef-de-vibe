import { ReactElement, ReactNode } from 'react';
import { EventMap, GutterType, ViewType, RenderToken, RenderGutter } from '../context';
import { ChangeData, HunkData } from '../utils';
import { HunkTokens } from '../tokenize';
export type DiffType = 'add' | 'delete' | 'modify' | 'rename' | 'copy';
export interface DiffProps {
    diffType: DiffType;
    hunks: HunkData[];
    viewType?: ViewType;
    gutterType?: GutterType;
    generateAnchorID?: (change: ChangeData) => string | undefined;
    selectedChanges?: string[];
    widgets?: Record<string, ReactNode>;
    optimizeSelection?: boolean;
    className?: string;
    hunkClassName?: string;
    lineClassName?: string;
    generateLineClassName?: (params: {
        changes: ChangeData[];
        defaultGenerate: () => string;
    }) => string | undefined;
    gutterClassName?: string;
    codeClassName?: string;
    tokens?: HunkTokens | null;
    renderToken?: RenderToken;
    renderGutter?: RenderGutter;
    gutterEvents?: EventMap;
    codeEvents?: EventMap;
    children?: (hunks: HunkData[]) => ReactElement | ReactElement[];
}
declare function Diff(props: DiffProps): import("react/jsx-runtime").JSX.Element;
declare const _default: import("react").MemoExoticComponent<typeof Diff>;
export default _default;
