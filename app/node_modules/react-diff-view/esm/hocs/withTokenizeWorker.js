import { jsx as _jsx } from "react/jsx-runtime";
import { useTokenizeWorker } from '../hooks';
import { wrapDisplayName } from './wrapDisplayName';
function defaultMapPayload(data) {
    return data;
}
export default function withTokenizeWorkerwithTokenizeWorker(worker, options = {}) {
    const { mapPayload = defaultMapPayload, ...hookOptions } = options;
    function resolveMessagePayload(props) {
        const { hunks, oldSource, language } = props;
        const input = { language, oldSource, hunks };
        return mapPayload(input, props);
    }
    return function wrap(ComponentIn) {
        function ComponentOut(props) {
            const payload = resolveMessagePayload(props);
            const tokenizationResult = useTokenizeWorker(worker, payload, hookOptions);
            return _jsx(ComponentIn, { ...props, ...tokenizationResult });
        }
        ComponentOut.displayName = wrapDisplayName(ComponentIn, 'withTokenizeWorker');
        return ComponentOut;
    };
}
//# sourceMappingURL=withTokenizeWorker.js.map