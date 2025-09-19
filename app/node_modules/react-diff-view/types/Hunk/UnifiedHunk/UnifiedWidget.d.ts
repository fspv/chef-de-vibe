import { ReactNode } from 'react';
export interface UnifiedWidgetProps {
    hideGutter: boolean;
    element: ReactNode;
}
export default function UnifiedWidget({ hideGutter, element }: UnifiedWidgetProps): import("react/jsx-runtime").JSX.Element;
