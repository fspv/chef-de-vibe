import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { Children } from 'react';
import classNames from 'classnames';
export default function SplitDecoration(props) {
    const { hideGutter, monotonous, className, gutterClassName, contentClassName, children } = props;
    const computedClassName = classNames('diff-decoration', className);
    const computedGutterClassName = classNames('diff-decoration-gutter', gutterClassName);
    const computedContentClassName = classNames('diff-decoration-content', contentClassName);
    const columnCount = (hideGutter ? 2 : 4) / (monotonous ? 2 : 1);
    const headerContentColSpan = columnCount - (hideGutter ? 0 : 1);
    // One element spans all gutter and content cells
    if (Children.count(children) === 1) {
        return (_jsx("tbody", { className: computedClassName, children: _jsx("tr", { children: _jsx("td", { colSpan: columnCount, className: computedContentClassName, children: children }) }) }));
    }
    const [gutter, content] = children;
    return (_jsx("tbody", { className: computedClassName, children: _jsxs("tr", { children: [!hideGutter && _jsx("td", { className: computedGutterClassName, children: gutter }), _jsx("td", { colSpan: headerContentColSpan, className: computedContentClassName, children: content })] }) }));
}
//# sourceMappingURL=SplitDecoration.js.map