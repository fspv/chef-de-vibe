import { ComponentType } from 'react';
import { HunkData, Source } from '../utils';
export default function withSourceExpansion(): <P extends {
    hunks: HunkData[];
    oldSource: Source;
}>(ComponentIn: ComponentType<P>) => {
    (props: P & {
        onExpandRange: (start: number, end: number) => void;
    }): import("react/jsx-runtime").JSX.Element;
    displayName: string;
};
