/// <reference types="react" />
import { ChangeData } from '../../utils';
import { TokenNode } from '../../tokenize';
import { ChangeSharedProps } from '../interface';
interface SplitChangeProps extends ChangeSharedProps {
    className: string;
    oldChange: ChangeData | null;
    newChange: ChangeData | null;
    oldSelected: boolean;
    newSelected: boolean;
    oldTokens: TokenNode[] | null;
    newTokens: TokenNode[] | null;
    monotonous: boolean;
}
declare function SplitChange(props: SplitChangeProps): import("react/jsx-runtime").JSX.Element;
declare const _default: import("react").MemoExoticComponent<typeof SplitChange>;
export default _default;
