import { jsx as _jsx } from "react/jsx-runtime";
import { useChangeSelect } from '../hooks';
import { wrapDisplayName } from './wrapDisplayName';
export default function withChangeSelect(options) {
    return function wrap(ComponentIn) {
        function ComponentOut(props) {
            const [selectedChanges, toggleChangeSelection] = useChangeSelect(props.hunks, options);
            return (_jsx(ComponentIn, { ...props, selectedChanges: selectedChanges, onToggleChangeSelection: toggleChangeSelection }));
        }
        ComponentOut.displayName = wrapDisplayName(ComponentIn, 'withChangeSelect');
        return ComponentOut;
    };
}
//# sourceMappingURL=withChangeSelect.js.map