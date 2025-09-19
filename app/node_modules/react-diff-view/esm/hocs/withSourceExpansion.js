import { jsx as _jsx } from "react/jsx-runtime";
import { useSourceExpansion } from '../hooks';
import { wrapDisplayName } from './wrapDisplayName';
export default function withSourceExpansion() {
    return function wrap(ComponentIn) {
        function ComponentOut(props) {
            const [renderingHunks, expandRange] = useSourceExpansion(props.hunks, props.oldSource);
            return (_jsx(ComponentIn, { ...props, hunks: renderingHunks, onExpandRange: expandRange }));
        }
        ComponentOut.displayName = wrapDisplayName(ComponentIn, 'withSourceExpansion');
        return ComponentOut;
    };
}
//# sourceMappingURL=withSourceExpansion.js.map