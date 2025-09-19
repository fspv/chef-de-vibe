import { Side } from '../../interface';
export declare function first<T>(array: T[]): T;
export declare function last<T>(array: T[]): T;
export declare function sideToProperty(side: Side): readonly ["oldStart" | "newStart", "oldLines" | "newLines"];
