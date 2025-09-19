import { ComponentType } from 'react';
import { DiffProps } from '../Diff';
import { Source } from '../utils';
export default function minCollapsedLines(minLinesExclusive: number): (ComponentIn: ComponentType<DiffProps>) => {
    (props: DiffProps & {
        oldSource: Source;
    }): import("react/jsx-runtime").JSX.Element;
    displayName: string;
};
