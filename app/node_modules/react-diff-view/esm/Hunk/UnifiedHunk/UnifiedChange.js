import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { memo, useState, useMemo, useCallback } from 'react';
import classNames from 'classnames';
import { mapValues } from 'lodash';
import { getChangeKey } from '../../utils';
import CodeCell from '../CodeCell';
import { composeCallback, renderDefaultBy, wrapInAnchorBy } from '../utils';
function useBoundCallbacks(callbacks, arg, hoverOn, hoverOff) {
    return useMemo(() => {
        const output = mapValues(callbacks, fn => (e) => fn && fn(arg, e));
        output.onMouseEnter = composeCallback(hoverOn, output.onMouseEnter);
        output.onMouseLeave = composeCallback(hoverOff, output.onMouseLeave);
        return output;
    }, [callbacks, hoverOn, hoverOff, arg]);
}
function useBoolean() {
    const [value, setValue] = useState(false);
    const on = useCallback(() => setValue(true), []);
    const off = useCallback(() => setValue(false), []);
    return [value, on, off];
}
function renderGutterCell(className, change, changeKey, side, gutterAnchor, anchorTarget, events, inHoverState, renderGutter) {
    const gutterOptions = {
        change,
        side,
        inHoverState,
        renderDefault: renderDefaultBy(change, side),
        wrapInAnchor: wrapInAnchorBy(gutterAnchor, anchorTarget),
    };
    return (_jsx("td", { className: className, ...events, "data-change-key": changeKey, children: renderGutter(gutterOptions) }));
}
function UnifiedChange(props) {
    const { change, selected, tokens, className, generateLineClassName, gutterClassName, codeClassName, gutterEvents, codeEvents, hideGutter, gutterAnchor, generateAnchorID, renderToken, renderGutter, } = props;
    const { type, content } = change;
    const changeKey = getChangeKey(change);
    const [hover, hoverOn, hoverOff] = useBoolean();
    const eventArg = useMemo(() => ({ change }), [change]);
    const boundGutterEvents = useBoundCallbacks(gutterEvents, eventArg, hoverOn, hoverOff);
    const boundCodeEvents = useBoundCallbacks(codeEvents, eventArg, hoverOn, hoverOff);
    const anchorID = generateAnchorID(change);
    const lineClassName = generateLineClassName({
        changes: [change],
        defaultGenerate: () => className,
    });
    const gutterClassNameValue = classNames('diff-gutter', `diff-gutter-${type}`, gutterClassName, { 'diff-gutter-selected': selected });
    const codeClassNameValue = classNames('diff-code', `diff-code-${type}`, codeClassName, { 'diff-code-selected': selected });
    return (_jsxs("tr", { id: anchorID, className: classNames('diff-line', lineClassName), children: [!hideGutter && renderGutterCell(gutterClassNameValue, change, changeKey, 'old', gutterAnchor, anchorID, boundGutterEvents, hover, renderGutter), !hideGutter && renderGutterCell(gutterClassNameValue, change, changeKey, 'new', gutterAnchor, anchorID, boundGutterEvents, hover, renderGutter), _jsx(CodeCell, { className: codeClassNameValue, changeKey: changeKey, text: content, tokens: tokens, renderToken: renderToken, ...boundCodeEvents })] }));
}
export default memo(UnifiedChange);
//# sourceMappingURL=UnifiedChange.js.map