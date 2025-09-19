import { ComponentType } from 'react';
import { UseChangeSelectOptions } from '../hooks';
import { ChangeEventArgs } from '../context';
import { HunkData } from '../utils';
export default function withChangeSelect(options: UseChangeSelectOptions): <P extends {
    hunks: HunkData[];
}>(ComponentIn: ComponentType<P>) => {
    (props: P & {
        onToggleChangeSelection: (args: ChangeEventArgs) => void;
    }): import("react/jsx-runtime").JSX.Element;
    displayName: string;
};
