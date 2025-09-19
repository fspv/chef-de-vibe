/// <reference types="react" />
import { ChangeData } from '../../utils';
import { TokenNode } from '../../tokenize';
import { ChangeSharedProps } from '../interface';
interface UnifiedChangeProps extends ChangeSharedProps {
    change: ChangeData;
    tokens: TokenNode[] | null;
    className: string;
    selected: boolean;
}
declare function UnifiedChange(props: UnifiedChangeProps): import("react/jsx-runtime").JSX.Element;
declare const _default: import("react").MemoExoticComponent<typeof UnifiedChange>;
export default _default;
