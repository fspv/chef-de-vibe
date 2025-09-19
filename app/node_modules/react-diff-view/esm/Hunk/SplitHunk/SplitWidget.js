import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
export default function SplitWidget({ hideGutter, oldElement, newElement, monotonous }) {
    if (monotonous) {
        return (_jsx("tr", { className: "diff-widget", children: _jsx("td", { colSpan: hideGutter ? 1 : 2, className: "diff-widget-content", children: oldElement || newElement }) }));
    }
    if (oldElement === newElement) {
        return (_jsx("tr", { className: "diff-widget", children: _jsx("td", { colSpan: hideGutter ? 2 : 4, className: "diff-widget-content", children: oldElement }) }));
    }
    return (_jsxs("tr", { className: "diff-widget", children: [_jsx("td", { colSpan: hideGutter ? 1 : 2, className: "diff-widget-content", children: oldElement }), _jsx("td", { colSpan: hideGutter ? 1 : 2, className: "diff-widget-content", children: newElement })] }));
}
//# sourceMappingURL=SplitWidget.js.map