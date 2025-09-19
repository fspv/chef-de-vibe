import { ReactNode } from 'react';
import { Side } from '../interface';
import { ChangeData } from '../utils';
export declare function renderDefaultBy(change: ChangeData, side: Side): () => ReactNode;
export declare function wrapInAnchorBy(gutterAnchor: boolean, anchorTarget: string | null | undefined): (element: ReactNode) => ReactNode;
export declare function composeCallback<E>(own: () => void, custom: ((e: E) => void) | undefined): (e: E) => void;
