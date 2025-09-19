import { jsx as _jsx } from "react/jsx-runtime";
import { useMinCollapsedLines } from '../hooks';
import { wrapDisplayName } from './wrapDisplayName';
export default function minCollapsedLines(minLinesExclusive) {
    return function wrap(ComponentIn) {
        function ComponentOut(props) {
            const renderingHunks = useMinCollapsedLines(minLinesExclusive, props.hunks, props.oldSource);
            return _jsx(ComponentIn, { ...props, hunks: renderingHunks });
        }
        ComponentOut.displayName = wrapDisplayName(ComponentIn, 'minCollapsedLines');
        return ComponentOut;
    };
}
//# sourceMappingURL=minCollapsedLines.js.map