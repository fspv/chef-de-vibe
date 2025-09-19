import { jsx as _jsx } from "react/jsx-runtime";
import { useDiffSettings } from '../context';
import UnifiedHunk from './UnifiedHunk';
import SplitHunk from './SplitHunk';
function Hunk({ hunk }) {
    const { gutterType, hunkClassName, ...context } = useDiffSettings();
    const hideGutter = gutterType === 'none';
    const gutterAnchor = gutterType === 'anchor';
    const RenderingHunk = context.viewType === 'unified' ? UnifiedHunk : SplitHunk;
    return (_jsx(RenderingHunk, { ...context, hunk: hunk, hideGutter: hideGutter, gutterAnchor: gutterAnchor, className: hunkClassName }));
}
export default Hunk;
//# sourceMappingURL=index.js.map