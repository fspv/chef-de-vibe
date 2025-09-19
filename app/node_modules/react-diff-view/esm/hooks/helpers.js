import { useReducer, useRef } from 'react';
function updateCollection(collection, command) {
    switch (command.type) {
        case 'push':
            return [...collection, command.value];
        case 'clear':
            return collection.length ? [] : collection;
        case 'toggle':
            return collection.includes(command.value)
                ? collection.filter(item => item !== command.value)
                : collection.concat(command.value);
        case 'only':
            return [command.value];
        default:
            return collection;
    }
}
export function useCollection() {
    const [collection, dispatch] = useReducer(updateCollection, []);
    return {
        collection,
        clear() {
            dispatch({ type: 'clear' });
        },
        push(value) {
            dispatch({ value, type: 'push' });
        },
        toggle(value) {
            dispatch({ value, type: 'toggle' });
        },
        only(value) {
            dispatch({ value, type: 'only' });
        },
    };
}
// This is actually a hack around the lack of custom comparator support in `useEffect` hook.
export function useCustomEqualIdentifier(value, equals) {
    const cache = useRef(undefined);
    const identifier = useRef(0);
    const isEqual = equals(value, cache.current);
    // TODO: this is not cocurrency safe
    if (!isEqual) {
        cache.current = value;
        identifier.current = identifier.current + 1;
    }
    return identifier.current;
}
//# sourceMappingURL=helpers.js.map