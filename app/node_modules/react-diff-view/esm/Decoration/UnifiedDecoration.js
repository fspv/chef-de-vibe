import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { Children } from 'react';
import classNames from 'classnames';
export default function UnifiedDecoration(props) {
    const { hideGutter, className, gutterClassName, contentClassName, children } = props;
    const computedClassName = classNames('diff-decoration', className);
    const computedGutterClassName = classNames('diff-decoration-gutter', gutterClassName);
    const computedContentClassName = classNames('diff-decoration-content', contentClassName);
    // One element spans all gutter and content cells
    if (Children.count(children) === 1) {
        return (_jsx("tbody", { className: computedClassName, children: _jsx("tr", { children: _jsx("td", { colSpan: hideGutter ? 1 : 3, className: computedContentClassName, children: children }) }) }));
    }
    const [gutter, content] = children;
    return (_jsx("tbody", { className: computedClassName, children: _jsxs("tr", { children: [!hideGutter && _jsx("td", { colSpan: 2, className: computedGutterClassName, children: gutter }), _jsx("td", { className: computedContentClassName, children: content })] }) }));
}
//# sourceMappingURL=UnifiedDecoration.js.map