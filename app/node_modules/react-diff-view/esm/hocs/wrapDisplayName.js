// Based on https://github.com/acdlite/recompose/blob/a255b23/src/packages/recompose/getDisplayName.js
function getDisplayName(Component) {
    return (typeof Component === 'string' || Component == null)
        ? Component
        : Component.displayName || Component.name || 'Component';
}
// based on https://github.com/acdlite/recompose/blob/d55575f/src/packages/recompose/wrapDisplayName.js
export function wrapDisplayName(BaseComponent, hocName) {
    return `${hocName}(${getDisplayName(BaseComponent)})`;
}
//# sourceMappingURL=wrapDisplayName.js.map