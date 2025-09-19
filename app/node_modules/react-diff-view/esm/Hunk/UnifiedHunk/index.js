import { jsx as _jsx } from "react/jsx-runtime";
import classNames from 'classnames';
import { getChangeKey, computeOldLineNumber, computeNewLineNumber, isDelete } from '../../utils';
import UnifiedChange from './UnifiedChange';
import UnifiedWidget from './UnifiedWidget';
function groupElements(changes, widgets) {
    return changes.reduce((elements, change) => {
        const key = getChangeKey(change);
        elements.push(['change', key, change]);
        const widget = widgets[key];
        if (widget) {
            elements.push(['widget', key, widget]);
        }
        return elements;
    }, []);
}
function renderRow([type, key, value], props) {
    const { hideGutter, selectedChanges, tokens, lineClassName, ...changeProps } = props;
    if (type === 'change') {
        const side = isDelete(value) ? 'old' : 'new';
        const lineNumber = isDelete(value) ? computeOldLineNumber(value) : computeNewLineNumber(value);
        const tokensOfLine = tokens ? tokens[side][lineNumber - 1] : null;
        return (_jsx(UnifiedChange, { className: lineClassName, change: value, hideGutter: hideGutter, selected: selectedChanges.includes(key), tokens: tokensOfLine, ...changeProps }, `change${key}`));
    }
    else if (type === 'widget') {
        return _jsx(UnifiedWidget, { hideGutter: hideGutter, element: value }, `widget${key}`);
    }
    return null;
}
export default function UnifiedHunk(props) {
    const { hunk, widgets, className, ...childrenProps } = props;
    const elements = groupElements(hunk.changes, widgets);
    return (_jsx("tbody", { className: classNames('diff-hunk', className), children: elements.map(element => renderRow(element, childrenProps)) }));
}
//# sourceMappingURL=index.js.map