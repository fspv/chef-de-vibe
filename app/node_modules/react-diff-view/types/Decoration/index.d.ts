import { ReactNode } from 'react';
export interface DecorationProps {
    className?: string;
    gutterClassName?: string;
    contentClassName?: string;
    children: ReactNode | [ReactNode, ReactNode];
}
export default function Decoration(props: DecorationProps): import("react/jsx-runtime").JSX.Element;
