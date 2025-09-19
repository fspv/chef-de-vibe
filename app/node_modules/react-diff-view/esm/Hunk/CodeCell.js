import { jsx as _jsx } from "react/jsx-runtime";
import { memo } from 'react';
import classNames from 'classnames';
const defaultRenderToken = ({ type, value, markType, properties, className, children }, i) => {
    const renderWithClassName = (className) => (_jsx("span", { className: className, children: value ? value : (children && children.map(defaultRenderToken)) }, i));
    switch (type) {
        case 'text':
            return value;
        case 'mark':
            return renderWithClassName(`diff-code-mark diff-code-mark-${markType}`);
        case 'edit':
            return renderWithClassName('diff-code-edit');
        default: {
            // properties normally not exist since it is deconstructed in pickRange, remove in next major release
            const legacyClassName = properties && properties.className;
            return renderWithClassName(classNames(className || legacyClassName));
        }
    }
};
function isEmptyToken(tokens) {
    if (!Array.isArray(tokens)) {
        return true;
    }
    if (tokens.length > 1) {
        return false;
    }
    if (tokens.length === 1) {
        const [token] = tokens;
        return token.type === 'text' && !token.value;
    }
    return true;
}
function CodeCell(props) {
    const { changeKey, text, tokens, renderToken, ...attributes } = props;
    const actualRenderToken = renderToken
        ? (token, i) => renderToken(token, defaultRenderToken, i)
        : defaultRenderToken;
    return (_jsx("td", { ...attributes, "data-change-key": changeKey, children: tokens
            ? (isEmptyToken(tokens) ? ' ' : tokens.map(actualRenderToken))
            : (text || ' ') }));
}
export default memo(CodeCell);
//# sourceMappingURL=CodeCell.js.map