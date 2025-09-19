export declare function useCollection<T>(): {
    collection: T[];
    clear(): void;
    push(value: T): void;
    toggle(value: T): void;
    only(value: T): void;
};
export declare function useCustomEqualIdentifier<T>(value: T, equals: (x: T, y: T | undefined) => boolean): number;
