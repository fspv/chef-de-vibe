import { HTMLAttributes } from 'react';
import { TokenNode } from '../tokenize';
import { RenderToken } from '../context';
export interface CodeCellProps extends HTMLAttributes<HTMLTableCellElement> {
    changeKey: string;
    text: string;
    tokens: TokenNode[] | null;
    renderToken: RenderToken | undefined;
}
declare function CodeCell(props: CodeCellProps): import("react/jsx-runtime").JSX.Element;
declare const _default: import("react").MemoExoticComponent<typeof CodeCell>;
export default _default;
