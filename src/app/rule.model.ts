export type Condition = {
    type: 'all' | 'any';
    conditions: Condition[];
} | {
    type: 'extension';
    values: string[];
} | {
    type: 'name_contains';
    value: string;
} | {
    type: 'size';
    min: number | null;
    max: number | null;
} | {
    type: 'modified_age' | 'created_age';
    seconds: number;
};
export interface Rule {
    id: number | null;
    name: string;
    source: string;
    condition: Condition;
    cron: string;
    enabled: boolean;
    action: 'MOVE' | 'SORT';
    destination: string;
    min_age_seconds: number;
}
export interface Run {
    id: number;
    rule_name: string;
    trigger: string;
    start_time: string;
    end_time: string | null;
    status: string;
    processed: number;
    skipped: number;
    failed: number;
    error: string | null;
}
export interface NextRun {
    rule_id: number;
    time: string | null;
    waiting: boolean;
    error: string | null;
}
export interface ActiveRun {
    run_id: number;
    rule_id: number;
    rule_name: string;
    stopping: boolean;
}
