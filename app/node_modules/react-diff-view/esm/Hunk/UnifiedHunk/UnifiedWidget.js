import { jsx as _jsx } from "react/jsx-runtime";
export default function UnifiedWidget({ hideGutter, element }) {
    return (_jsx("tr", { className: "diff-widget", children: _jsx("td", { colSpan: hideGutter ? 1 : 3, className: "diff-widget-content", children: element }) }));
}
//# sourceMappingURL=UnifiedWidget.js.map