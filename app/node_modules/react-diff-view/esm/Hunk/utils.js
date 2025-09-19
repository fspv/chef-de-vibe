import { jsx as _jsx } from "react/jsx-runtime";
import { computeOldLineNumber, computeNewLineNumber } from '../utils';
export function renderDefaultBy(change, side) {
    return () => {
        const lineNumber = side === 'old' ? computeOldLineNumber(change) : computeNewLineNumber(change);
        return lineNumber === -1 ? undefined : lineNumber;
    };
}
export function wrapInAnchorBy(gutterAnchor, anchorTarget) {
    return (element) => {
        if (!gutterAnchor || !element) {
            return element;
        }
        return _jsx("a", { href: anchorTarget ? '#' + anchorTarget : undefined, children: element });
    };
}
export function composeCallback(own, custom) {
    if (custom) {
        return (e) => {
            own();
            custom(e); // `custom` is already bound with `arg`
        };
    }
    return own;
}
//# sourceMappingURL=utils.js.map