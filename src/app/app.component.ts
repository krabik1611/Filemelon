import { Component, OnInit, OnDestroy, signal, computed, ViewChild, ElementRef } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { Condition, Rule, Run, NextRun, ActiveRun } from './rule.model';
type UiSnapshot = { rules: Rule[]; history: Run[]; next: NextRun[]; active: ActiveRun | null };
@Component({ selector: 'app-root', imports: [FormsModule], templateUrl: './app.component.html', styleUrl: './app.component.css' })
export class AppComponent implements OnInit, OnDestroy {
    compact = signal(this.preference('filemelon.compact', false));
    sidebarHidden = signal(this.preference('filemelon.sidebarHidden', false));
    draggedRuleId: number | null = null;
    private preference(key: string, fallback: boolean) { try { const value = localStorage.getItem(key); return value === null ? fallback : value === 'true'; } catch { return fallback; } }
    setCompact(value: boolean) { this.compact.set(value); try { localStorage.setItem('filemelon.compact', String(value)); } catch {} }
    toggleSidebar() { const value = !this.sidebarHidden(); this.sidebarHidden.set(value); try { localStorage.setItem('filemelon.sidebarHidden', String(value)); } catch {} }
    async setAll(enabled: boolean) { await this.act(async () => { await invoke('set_all_rules_enabled', { enabled }); await this.refresh(); }); }
    async reorder(sourceId: number, targetId: number) { if (sourceId === targetId) return; const ids = this.rules().map(r => r.id!); const from = ids.indexOf(sourceId), to = ids.indexOf(targetId); if (from < 0 || to < 0) return; ids.splice(from, 1); ids.splice(ids.indexOf(targetId), 0, sourceId); await this.act(async () => { await invoke('reorder_rules', { ids }); await this.refresh(); }); }
    startDrag(event: DragEvent, rule: Rule) { if (rule.id === null || this.busy() || !!this.active()) { event.preventDefault(); return; } this.draggedRuleId = rule.id; event.dataTransfer?.setData('text/plain', String(rule.id)); if (event.dataTransfer) { event.dataTransfer.effectAllowed = 'move'; event.dataTransfer.dropEffect = 'move'; } }
    dragOver(event: DragEvent) { if (this.draggedRuleId !== null) { event.preventDefault(); if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'; } }
    async dropDrag(event: DragEvent, rule: Rule) { event.preventDefault(); const source = this.draggedRuleId; this.draggedRuleId = null; if (source !== null && rule.id !== null) await this.reorder(source, rule.id); }
    endDrag() { this.draggedRuleId = null; }
    view = signal<'rules' | 'history'>('rules');
    search = signal('');
    ruleFilter = signal<'all' | 'enabled' | 'disabled'>('all');
    enabledCount = computed(() => this.rules().filter(r => r.enabled).length);
    visibleRules = computed(() => {
        const query = this.search().trim().toLowerCase();
        return this.rules().filter(r => (this.ruleFilter() === 'all' || r.enabled === (this.ruleFilter() === 'enabled')) && (!query || [r.name, r.source, r.destination, this.conditionLabel(r.condition)].some(value => value.toLowerCase().includes(query))));
    });
    statusLabel(status: string) {
        return ({ running: 'Выполняется', success: 'Завершено', partial: 'С ошибками', failed: 'Ошибка', interrupted: 'Прервано', cancelled: 'Остановлено' } as Record<string, string>)[status] || status;
    }
    conditionChips(condition: Condition) {
        return condition.type === 'all' || condition.type === 'any' ? condition.conditions.map(c => this.conditionLabel(c)) : [this.conditionLabel(condition)];
    }
    resetFilters() {
        this.search.set('');
        this.ruleFilter.set('all');
    }
    schedulePreset = 'hourly';
    schedulePresets = [{ value: 'five', label: 'Каждые 5 минут', cron: '*/5 * * * *' }, { value: 'hourly', label: 'Каждый час', cron: '0 * * * *' }, { value: 'daily', label: 'Каждый день в 00:00', cron: '0 0 * * *' }, { value: 'custom', label: 'Своё расписание', cron: '' }];
    changeSchedule(value: string) {
        this.schedulePreset = value;
        const preset = this.schedulePresets.find(p => p.value === value);
        if (preset?.cron)
            this.draft.cron = preset.cron;
    }
    addTemplate(token: string) {
        const separator = this.draft.destination.endsWith('\\') || this.draft.destination.endsWith('/') ? '' : '\\';
        this.draft.destination += separator + '{' + token + '}';
    }
    @ViewChild('editor')
    editor!: ElementRef<HTMLDialogElement>;
    nextRuns = signal(new Map<number, NextRun>());
    notice = signal('');
    timeUnits = [{ value: 1, label: 'сек.' }, { value: 60, label: 'мин.' }, { value: 3600, label: 'ч.' }, { value: 86400, label: 'дн.' }];
    sizeUnits = [{ value: 1, label: 'Б' }, { value: 1024, label: 'КБ' }, { value: 1024 ** 2, label: 'МБ' }, { value: 1024 ** 3, label: 'ГБ' }, { value: 1024 ** 4, label: 'ТБ' }];
    minSizeUnit = 1024 ** 2;
    maxSizeUnit = 1024 ** 2;
    modifiedAgeUnit = 60;
    createdAgeUnit = 60;
    stabilityAge = 1;
    stabilityUnit = 86400;
    formatSize(bytes: number) {
        const unit = [...this.sizeUnits].reverse().find(u => bytes >= u.value) || this.sizeUnits[0];
        return `${(bytes / unit.value).toLocaleString('ru-RU', { maximumFractionDigits: 3 })} ${unit.label}`;
    }
    nextRunLabel(rule: Rule) {
        if (!rule.enabled)
            return 'Выключено';
        const next = this.nextRuns().get(rule.id!);
        if (!next)
            return 'Расчёт расписания…';
        if (next.error)
            return next.error;
        if (!next.time)
            return 'Нет будущих запусков';
        return this.formatDate(next.time) + (next.waiting ? ' · ожидает завершения текущего запуска' : '');
    }
    private displayUnit(value: number, units: {
        value: number;
        label: string;
    }[], fallback: number): [
        number,
        number
    ] {
        const unit = value === 0 ? fallback : ([...units].reverse().find(u => value % u.value === 0)?.value || 1);
        return [value / unit, unit];
    }
    private scaled(value: number, unit: number): number {
        const result = Math.round(value * unit);
        if (!Number.isFinite(value) || value < 0 || !Number.isSafeInteger(result))
            throw new Error('Укажите неотрицательное значение с точностью до байта или секунды.');
        return result;
    }
    async browse(field: 'source' | 'destination') {
        await this.act(async () => {
            const current = this.draft[field];
            const selected = await invoke<string | null>('pick_directory', { current });
            if (selected === null)
                return;
            let suffix = '';
            if (field === 'destination' && this.draft.action === 'SORT' && current.includes('{')) {
                const token = current.indexOf('{');
                const separator = Math.max(current.lastIndexOf('\\', token), current.lastIndexOf('/', token));
                suffix = current.slice(separator + 1);
            }
            this.draft[field] = selected + (suffix ? '\\' + suffix : '');
        });
    }
    async exportRules() {
        await this.act(async () => {
            const path = await invoke<string | null>('export_rules');
            if (path)
                this.notice.set(`Правила экспортированы: ${path}`);
        });
    }
    async importRules() {
        await this.act(async () => {
            const result = await invoke<{
                imported: number;
            } | null>('import_rules');
            if (result) {
                this.notice.set(`Добавлено правил: ${result.imported}. Они выключены — проверьте пути перед включением.`);
                await this.refresh();
            }
        });
    }
    pendingDelete = signal<number | null>(null);
    private dates = new Intl.DateTimeFormat('ru-RU', { day: '2-digit', month: 'long', year: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit' });
    formatDate(value: string | null) {
        if (!value)
            return '—';
        const date = new Date(value);
        return Number.isNaN(date.getTime()) ? '—' : this.dates.format(date);
    }
    formatAge(seconds: number) {
        if (seconds === 0)
            return '0 сек.';
        for (const [unit, label] of [[86400, 'дн.'], [3600, 'ч.'], [60, 'мин.']] as const) {
            if (seconds % unit === 0)
                return `${seconds / unit} ${label}`;
        }
        return `${seconds} сек.`;
    }
    conditionLabel(condition: Condition): string {
        switch (condition.type) {
            case 'all':
            case 'any': return condition.conditions.length ? condition.conditions.map(c => `(${this.conditionLabel(c)})`).join(condition.type === 'all' ? ' И ' : ' ИЛИ ') : (condition.type === 'all' ? 'Все файлы' : 'Нет подходящих файлов');
            case 'extension': return `Расширение: ${condition.values.map(v => '.' + v.replace(/^\./, '')).join(', ')}`;
            case 'name_contains': return `Имя содержит «${condition.value}»`;
            case 'size': return [condition.min !== null ? `Размер от ${this.formatSize(condition.min)}` : '', condition.max !== null ? `до ${this.formatSize(condition.max)}` : ''].filter(Boolean).join(' ') || 'Любой размер';
            case 'modified_age': return `Изменён не менее ${this.formatAge(condition.seconds)} назад`;
            case 'created_age': return `Создан не менее ${this.formatAge(condition.seconds)} назад`;
        }
    }
    newRule() {
        this.schedulePreset = 'hourly';
        this.reset();
        this.error.set('');
        this.editor.nativeElement.showModal();
    }
    closeEditor() {
        if (!this.busy()) {
            this.editor.nativeElement.close();
            this.error.set('');
            this.reset();
        }
    }
    private backdropPress = false;
    editorPointerDown(event: PointerEvent) {
        this.backdropPress = this.isEditorBackdrop(event);
    }
    editorClick(event: MouseEvent) {
        const startedOutside = this.backdropPress;
        this.backdropPress = false;
        if (startedOutside && this.isEditorBackdrop(event))
            this.closeEditor();
    }
    private isEditorBackdrop(event: MouseEvent) {
        const dialog = this.editor.nativeElement;
        const rect = dialog.getBoundingClientRect();
        return event.target === dialog && (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom);
    }
    cancelEditor(event: Event) {
        event.preventDefault();
        this.closeEditor();
    }
    async deleteRule(rule: Rule) {
        if (rule.enabled)
            return;
        await this.act(async () => {
            await invoke('delete_rule', { id: rule.id });
            this.pendingDelete.set(null);
            await this.refresh();
        });
    }
    rules = signal<Rule[]>([]);
    history = signal<Run[]>([]);
    error = signal('');
    busy = signal(false);
    autostart = signal(false);
    active = signal<ActiveRun | null>(null);
    stopPending = signal(false);
    draft: Rule = this.empty();
    extensions = '';
    contains = '';
    minSize: number | null = null;
    maxSize: number | null = null;
    conditionMode: 'all' | 'any' = 'all';
    modifiedAge: number | null = null;
    createdAge: number | null = null;
    private unlisten?: UnlistenFn;
    empty(): Rule {
        return { id: null, name: '', source: '', condition: { type: 'all', conditions: [] }, cron: '0 * * * *', enabled: true, action: 'MOVE', destination: '', min_age_seconds: 86400 };
    }
    async ngOnInit() {
        if (!isTauri()) {
            this.error.set('Чтобы работать с файлами, откройте приложение Filemelon на компьютере.');
            return;
        }
        await this.act(async () => {
            await this.refresh();
            this.autostart.set(await invoke<boolean>('autostart_enabled'));
        });
        this.unlisten = await listen<UiSnapshot>('filemelon-state-changed', event => this.applySnapshot(event.payload));
    }
    ngOnDestroy() {
        this.unlisten?.();
    }
    private applySnapshot(snapshot: UiSnapshot) {
        this.rules.set(snapshot.rules);
        this.history.set(snapshot.history);
        this.nextRuns.set(new Map(snapshot.next.map(n => [n.rule_id, n])));
        this.active.set(snapshot.active);
    }
    async stop() {
        const run = this.active();
        if (!run || this.stopPending())
            return;
        this.stopPending.set(true);
        try {
            await invoke('stop_run', { runId: run.run_id });
            this.active.update(current => current?.run_id === run.run_id ? { ...current, stopping: true } : current);
        }
        catch (e) {
            await this.refresh();
            if (this.active()?.run_id === run.run_id)
                this.error.set(String(e));
        }
        finally {
            this.stopPending.set(false);
        }
    }
    async refresh() {
        const [rules, history, next] = await Promise.all([invoke<Rule[]>('list_rules'), invoke<Run[]>('run_history'), invoke<NextRun[]>('next_runs')]);
        this.rules.set(rules);
        this.history.set(history);
        this.nextRuns.set(new Map(next.map(n => [n.rule_id, n])));
    }
    async act(action: () => Promise<unknown>) {
        if (this.busy())
            return;
        this.busy.set(true);
        this.error.set('');
        try {
            await action();
        }
        catch (e) {
            this.error.set(String(e));
        }
        finally {
            this.busy.set(false);
        }
    }
    reset() {
        this.draft = this.empty();
        this.extensions = '';
        this.contains = '';
        this.minSize = null;
        this.maxSize = null;
        this.modifiedAge = null;
        this.createdAge = null;
        this.conditionMode = 'all';
        this.minSizeUnit = 1024 ** 2;
        this.maxSizeUnit = 1024 ** 2;
        this.modifiedAgeUnit = 60;
        this.createdAgeUnit = 60;
        this.stabilityAge = 1;
        this.stabilityUnit = 86400;
    }
    edit(rule: Rule) {
        const root = rule.condition;
        const children = root.type === 'all' || root.type === 'any' ? root.conditions : [root];
        if (children.some(c => c.type === 'all' || c.type === 'any') || new Set(children.map(c => c.type)).size !== children.length) {
            this.error.set('Это правило содержит вложенные группы. Изменение такого AST пока доступно только через Rust/API.');
            return;
        }
        this.reset();
        this.draft = structuredClone(rule);
        this.schedulePreset = this.schedulePresets.find(p => p.cron === rule.cron)?.value || 'custom';
        this.conditionMode = root.type === 'any' ? 'any' : 'all';
        for (const c of children) {
            switch (c.type) {
                case 'extension':
                    this.extensions = c.values.join(', ');
                    break;
                case 'name_contains':
                    this.contains = c.value;
                    break;
                case 'size':
                    this.minSize = c.min;
                    this.maxSize = c.max;
                    break;
                case 'modified_age':
                    this.modifiedAge = c.seconds;
                    break;
                case 'created_age':
                    this.createdAge = c.seconds;
                    break;
            }
        }
        if (this.minSize !== null)
            [this.minSize, this.minSizeUnit] = this.displayUnit(this.minSize, this.sizeUnits, 1024 ** 2);
        if (this.maxSize !== null)
            [this.maxSize, this.maxSizeUnit] = this.displayUnit(this.maxSize, this.sizeUnits, 1024 ** 2);
        if (this.modifiedAge !== null)
            [this.modifiedAge, this.modifiedAgeUnit] = this.displayUnit(this.modifiedAge, this.timeUnits, 60);
        if (this.createdAge !== null)
            [this.createdAge, this.createdAgeUnit] = this.displayUnit(this.createdAge, this.timeUnits, 60);
        [this.stabilityAge, this.stabilityUnit] = this.displayUnit(rule.min_age_seconds, this.timeUnits, 60);
        this.error.set('');
        this.editor.nativeElement.showModal();
    }
    async save() {
        await this.act(async () => {
            const conditions: Condition[] = [];
            if (this.extensions.trim())
                conditions.push({ type: 'extension', values: this.extensions.split(',').map(v => v.trim()).filter(Boolean) });
            if (this.contains.trim())
                conditions.push({ type: 'name_contains', value: this.contains.trim() });
            if (this.minSize !== null || this.maxSize !== null)
                conditions.push({ type: 'size', min: this.minSize === null ? null : this.scaled(this.minSize, this.minSizeUnit), max: this.maxSize === null ? null : this.scaled(this.maxSize, this.maxSizeUnit) });
            if (this.modifiedAge !== null)
                conditions.push({ type: 'modified_age', seconds: this.scaled(this.modifiedAge, this.modifiedAgeUnit) });
            if (this.createdAge !== null)
                conditions.push({ type: 'created_age', seconds: this.scaled(this.createdAge, this.createdAgeUnit) });
            if (this.conditionMode === 'any' && conditions.length === 0)
                throw new Error('Для «любого условия» добавьте хотя бы одно условие.');
            await invoke('save_rule', { rule: { ...this.draft, min_age_seconds: this.scaled(this.stabilityAge, this.stabilityUnit), condition: { type: this.conditionMode, conditions } } });
            this.editor.nativeElement.close();
            this.reset();
            await this.refresh();
        });
    }
    async toggle(rule: Rule) {
        await this.act(async () => {
            await invoke('set_rule_enabled', { id: rule.id, enabled: !rule.enabled });
            this.pendingDelete.set(null);
            await this.refresh();
        });
    }
    async run(rule: Rule) {
        await this.act(async () => {
            await invoke('run_now', { id: rule.id });
            await this.refresh();
        });
    }
    async toggleAutostart() {
        await this.act(async () => {
            await invoke('set_autostart', { enabled: !this.autostart() });
            this.autostart.set(await invoke<boolean>('autostart_enabled'));
        });
    }
}
