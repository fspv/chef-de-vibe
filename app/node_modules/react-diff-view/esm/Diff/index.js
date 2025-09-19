import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { memo, useRef, useCallback, useMemo } from 'react';
import classNames from 'classnames';
import { Provider, DEFAULT_CONTEXT_VALUE, } from '../context';
import Hunk from '../Hunk';
function noop() { }
function findClosest(target, className) {
    let current = target;
    while (current && current !== document.documentElement && !current.classList.contains(className)) {
        current = current.parentElement;
    }
    return current === document.documentElement ? null : current;
}
function setUserSelectStyle(element, selectable) {
    const value = selectable ? 'auto' : 'none';
    if (element instanceof HTMLElement && element.style.userSelect !== value) {
        element.style.userSelect = value; // eslint-disable-line no-param-reassign
    }
}
function defaultRenderChildren(hunks) {
    const key = (hunk) => `-${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines}`;
    return hunks.map(hunk => _jsx(Hunk, { hunk: hunk }, key(hunk)));
}
function Diff(props) {
    const { diffType, hunks, optimizeSelection, className, hunkClassName = DEFAULT_CONTEXT_VALUE.hunkClassName, lineClassName = DEFAULT_CONTEXT_VALUE.lineClassName, generateLineClassName = DEFAULT_CONTEXT_VALUE.generateLineClassName, gutterClassName = DEFAULT_CONTEXT_VALUE.gutterClassName, codeClassName = DEFAULT_CONTEXT_VALUE.codeClassName, gutterType = DEFAULT_CONTEXT_VALUE.gutterType, viewType = DEFAULT_CONTEXT_VALUE.viewType, gutterEvents = DEFAULT_CONTEXT_VALUE.gutterEvents, codeEvents = DEFAULT_CONTEXT_VALUE.codeEvents, generateAnchorID = DEFAULT_CONTEXT_VALUE.generateAnchorID, selectedChanges = DEFAULT_CONTEXT_VALUE.selectedChanges, widgets = DEFAULT_CONTEXT_VALUE.widgets, renderGutter = DEFAULT_CONTEXT_VALUE.renderGutter, tokens, renderToken, children = defaultRenderChildren, } = props;
    const root = useRef(null);
    const enableColumnSelection = useCallback(({ target, button }) => {
        if (button !== 0) {
            return;
        }
        const closestCell = findClosest(target, 'diff-code');
        if (!closestCell || !closestCell.parentElement) {
            return;
        }
        const selection = window.getSelection();
        if (selection) {
            selection.removeAllRanges();
        }
        const index = [...closestCell.parentElement.children].indexOf(closestCell);
        if (index !== 1 && index !== 3) {
            return;
        }
        const lines = root.current ? root.current.querySelectorAll('.diff-line') : [];
        for (const line of lines) {
            const cells = line.children;
            setUserSelectStyle(cells[1], index === 1);
            setUserSelectStyle(cells[3], index === 3);
        }
    }, []);
    const hideGutter = gutterType === 'none';
    const monotonous = diffType === 'add' || diffType === 'delete';
    const onTableMouseDown = (viewType === 'split' && !monotonous && optimizeSelection) ? enableColumnSelection : noop;
    const cols = useMemo(() => {
        if (viewType === 'unified') {
            return (_jsxs("colgroup", { children: [!hideGutter && _jsx("col", { className: "diff-gutter-col" }), !hideGutter && _jsx("col", { className: "diff-gutter-col" }), _jsx("col", {})] }));
        }
        if (monotonous) {
            return (_jsxs("colgroup", { children: [!hideGutter && _jsx("col", { className: "diff-gutter-col" }), _jsx("col", {})] }));
        }
        return (_jsxs("colgroup", { children: [!hideGutter && _jsx("col", { className: "diff-gutter-col" }), _jsx("col", {}), !hideGutter && _jsx("col", { className: "diff-gutter-col" }), _jsx("col", {})] }));
    }, [viewType, monotonous, hideGutter]);
    // TODO: in later versions, we can split context into multiple to reduce component render
    const settingsContextValue = useMemo(() => {
        return {
            hunkClassName,
            lineClassName,
            generateLineClassName,
            gutterClassName,
            codeClassName,
            monotonous,
            hideGutter,
            viewType,
            gutterType,
            codeEvents,
            gutterEvents,
            generateAnchorID,
            selectedChanges,
            widgets,
            renderGutter,
            tokens,
            renderToken,
        };
    }, [
        codeClassName,
        codeEvents,
        generateAnchorID,
        gutterClassName,
        gutterEvents,
        gutterType,
        hideGutter,
        hunkClassName,
        lineClassName,
        generateLineClassName,
        monotonous,
        renderGutter,
        renderToken,
        selectedChanges,
        tokens,
        viewType,
        widgets,
    ]);
    return (_jsx(Provider, { value: settingsContextValue, children: _jsxs("table", { ref: root, className: classNames('diff', `diff-${viewType}`, className), onMouseDown: onTableMouseDown, children: [cols, children(hunks)] }) }));
}
export default memo(Diff);
//# sourceMappingURL=index.js.map