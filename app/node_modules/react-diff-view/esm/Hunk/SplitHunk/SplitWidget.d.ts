import { ReactNode } from 'react';
export interface SplitWidgetProps {
    hideGutter: boolean;
    oldElement: ReactNode | null;
    newElement: ReactNode | null;
    monotonous: boolean;
}
export default function SplitWidget({ hideGutter, oldElement, newElement, monotonous }: SplitWidgetProps): import("react/jsx-runtime").JSX.Element;
