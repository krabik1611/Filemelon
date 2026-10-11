import { Component, OnInit, OnDestroy, signal, computed, ViewChild, ElementRef } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { Condition, Rule, Run, NextRun, ActiveRun } from './rule.model';
type Language = 'en' | 'ru';
const TEXT: Record<Language, Record<string, string>> = {
    en: {
        brandTagline: 'Order without effort', navRules: 'Rules', navHistory: 'History', background: 'Running in background', processing: 'Processing files', autostart: 'Start with Windows', trayHint: 'Closing the window keeps Filemelon in the tray.', menuShow: 'Show menu', menuHide: 'Hide menu', eyebrow: 'YOUR LOCAL FILE ORGANIZER', rulesTitle: 'Rules for your files', historyTitle: 'Run history', rulesSubtitle: 'Set it up once. Filemelon handles the rest.', historySubtitle: 'Results and details for every run.', newRule: 'New rule', hide: 'Hide', search: 'Search by name, folder or filter', import: '↓ Import', export: '↑ Export', disableAll: 'Disable all', enableAll: 'Enable all', viewToggle: 'Toggle view', all: 'All', enabled: 'Enabled', disabled: 'Disabled', priorityTip: 'The first matching rule has priority', move: 'Move', copy: 'Copy and synchronize', sort: 'Sort', deleteAction: 'Move to Recycle Bin', from: 'FROM', to: 'TO', action: 'ACTION', recycleBin: 'Recycle Bin', conditions: 'Conditions', nextRun: 'Next run', stability: 'No changes for at least', stableHint: 'For safety, files must be stable for at least 2 seconds', stableShort: 'stable >= 2 sec.', runNow: 'Run now', edit: 'Edit', delete: 'Delete', deleteConfirm: 'Delete “{name}”?', deleteHistory: 'The rule and its run history will be deleted.', deleteRule: 'Delete rule', cancel: 'Cancel', nothingFound: 'Nothing found', startOrder: 'This is where order begins', tryAnother: 'Try another name, path or filter.', firstRule: 'Choose a folder and create your first rule.', resetSearch: 'Clear search', noRuns: 'No runs yet', runRuleHint: 'Run a rule manually or wait for its schedule.', goRules: 'Go to rules', latestRuns: 'Latest runs', computerTime: 'Your computer time', rule: 'Rule', time: 'Time', status: 'Status', processed: 'Processed', skippedErrors: 'Skipped / errors', manual: 'Manual run', scheduled: 'Scheduled', until: 'Until ', details: 'Details', setup: 'AUTOMATION SETUP', editRule: 'Edit rule', newRuleTitle: 'New rule', basic: 'Basics', ruleName: 'Rule name', ruleNamePlaceholder: 'For example, sort videos', sourceFolder: 'Source folder', choose: 'Choose…', subfolders: 'Include subfolders', sourceHint: 'By default, only files directly in the selected folder are processed.', whatToDo: 'Action', actionLabel: 'Action', moveOption: 'Move to folder', copyOption: 'Copy and synchronize', sortOption: 'Sort using a path template', deleteOption: 'Move to Recycle Bin', destinationTemplate: 'Destination folder and template', destinationFolder: 'Destination folder', destinationPlaceholder: 'Choose a folder or enter a full path', copyHint: 'Source files stay in place. New and changed files are updated at the destination.', sortHint: 'Add date or extension subfolders. The file name is preserved.', whichFiles: 'Files', matchMode: 'Match conditions', allConditions: 'All conditions', anyCondition: 'Any condition', extensions: 'Extensions', extensionsPlaceholder: 'mp4, mkv, mov', extensionsHint: 'Comma-separated; the dot is optional', nameContains: 'Name contains', anyPart: 'Any part of the name', sizeAge: 'File size and age', optional: 'optional', minSize: 'Minimum size', maxSize: 'Maximum size', minUnit: 'Minimum size unit', maxUnit: 'Maximum size unit', bytesHint: '1 KB = 1024 B. Empty fields do not limit size.', modifiedAge: 'Not modified for at least', createdAge: 'Created at least', modifiedUnit: 'Modified age unit', createdUnit: 'Created age unit', noConditions: 'With no conditions, “All conditions” selects every file.', when: 'Schedule', schedule: 'Schedule', cron: 'Cron expression', cronHint: 'Minute, hour, day, month, day of week. Local time is used.', stabilityPeriod: 'Do not touch recently changed files', stabilityHint: 'Even with 0, a file must be stable for at least 2 seconds.', enableAfter: 'Enable this rule after saving', save: 'Save rule', saving: 'Saving…', closeEditor: 'Close editor', stop: 'Stop', stopping: 'Stopping…', running: 'Running', completed: 'Completed', partial: 'Completed with errors', failed: 'Error', interrupted: 'Interrupted', cancelled: 'Stopped', disabledLabel: 'Disabled', calculating: 'Calculating schedule…', noFuture: 'No future runs', waiting: ' · waiting for the current run to finish', allFiles: 'All files', noMatching: 'No matching files', extension: 'Extension', nameHas: 'Name contains', sizeFrom: 'Size from', sizeTo: 'to', anySize: 'Any size', modifiedFor: 'Not modified for', createdFor: 'Created at least', ago: ' ago', seconds: 'sec.', minutes: 'min.', hours: 'hr.', days: 'days', bytes: 'B', kb: 'KB', mb: 'MB', gb: 'GB', tb: 'TB', five: 'Every 5 minutes', hourly: 'Every hour', daily: 'Every day at 00:00', custom: 'Custom schedule', exported: 'Rules exported: {path}', imported: 'Rules added: {count}. They are disabled — check the paths before enabling them.', appOnly: 'Open the Filemelon desktop app to work with files.', nestedAst: 'This rule contains nested groups. Editing this AST is currently available only through Rust/API.', anyNeeds: 'Add at least one condition for “Any condition”.', enableRule: 'Enable rule', language: 'Language', english: 'English', russian: 'Russian'
    },
    ru: {
        brandTagline: 'Порядок без усилий', navRules: 'Правила', navHistory: 'История', background: 'Работает в фоне', processing: 'Обработка файлов', autostart: 'Автозапуск Windows', trayHint: 'Закрытие окна оставляет Filemelon в трее.', menuShow: 'Показать меню', menuHide: 'Скрыть меню', eyebrow: 'ВАШ ЛОКАЛЬНЫЙ ОРГАНАЙЗЕР', rulesTitle: 'Правила для ваших файлов', historyTitle: 'История запусков', rulesSubtitle: 'Настройте один раз. Filemelon позаботится об остальном.', historySubtitle: 'Результаты обработки и детали каждого запуска.', newRule: 'Новое правило', hide: 'Скрыть', search: 'Поиск по имени, папке или фильтру', import: '↓ Импорт', export: '↑ Экспорт', disableAll: 'Выключить все', enableAll: 'Включить все', viewToggle: 'Переключить вид', all: 'Все', enabled: 'Включены', disabled: 'Выключены', priorityTip: 'Первое подходящее правило имеет приоритет', move: 'Перемещение', copy: 'Копирование', sort: 'Сортировка', deleteAction: 'Удаление в корзину', from: 'ИЗ', to: 'В', action: 'ДЕЙСТВИЕ', recycleBin: 'Корзина', conditions: 'Условия', nextRun: 'Следующий запуск', stability: 'Не изменялся минимум', stableHint: 'Для защиты недавно записанных файлов минимальный период стабильности составляет 2 секунды', stableShort: 'стабильность >= 2 сек.', runNow: 'Запустить сейчас', edit: 'Редактировать', delete: 'Удалить', deleteConfirm: 'Удалить «{name}»?', deleteHistory: 'Правило и история его запусков будут удалены.', deleteRule: 'Удалить правило', cancel: 'Отмена', nothingFound: 'Ничего не найдено', startOrder: 'Здесь начинается порядок', tryAnother: 'Попробуйте другое название, путь или фильтр.', firstRule: 'Выберите папку и настройте первое правило для своих файлов.', resetSearch: 'Сбросить поиск', noRuns: 'Запусков пока нет', runRuleHint: 'Запустите правило вручную или дождитесь расписания.', goRules: 'Перейти к правилам', latestRuns: 'Последние запуски', computerTime: 'Время вашего компьютера', rule: 'Правило', time: 'Время', status: 'Статус', processed: 'Обработано', skippedErrors: 'Пропущено / ошибки', manual: 'Ручной запуск', scheduled: 'По расписанию', until: 'До ', details: 'Подробнее', setup: 'НАСТРОЙКА АВТОМАТИЗАЦИИ', editRule: 'Редактировать правило', newRuleTitle: 'Новое правило', basic: 'Основное', ruleName: 'Название правила', ruleNamePlaceholder: 'Например, сортировать видео', sourceFolder: 'Исходная папка', choose: 'Выбрать…', subfolders: 'Учитывать вложенные папки', sourceHint: 'По умолчанию обрабатываются файлы только из выбранной папки.', whatToDo: 'Что сделать', actionLabel: 'Действие', moveOption: 'Переместить в папку', copyOption: 'Копировать и синхронизировать', sortOption: 'Сортировать по шаблону пути', deleteOption: 'Удалить в корзину', destinationTemplate: 'Папка назначения и шаблон', destinationFolder: 'Папка назначения', destinationPlaceholder: 'Выберите папку или введите полный путь', copyHint: 'Файлы остаются в исходной папке. Новые и изменённые файлы обновляются в назначении.', sortHint: 'Добавьте подпапки по дате или расширению. Имя файла сохраняется.', whichFiles: 'Какие файлы', matchMode: 'Совпадение условий', allConditions: 'Все условия', anyCondition: 'Хотя бы одно условие', extensions: 'Расширения', extensionsPlaceholder: 'mp4, mkv, mov', extensionsHint: 'Через запятую, точка не обязательна', nameContains: 'Имя содержит', anyPart: 'Любая часть имени', sizeAge: 'Размер и возраст файла', optional: 'необязательно', minSize: 'Минимальный размер', maxSize: 'Максимальный размер', minUnit: 'Единица минимального размера', maxUnit: 'Единица максимального размера', bytesHint: '1 КБ = 1024 Б. Пустые поля не ограничивают размер.', modifiedAge: 'Изменён не менее чем назад', createdAge: 'Создан не менее чем назад', modifiedUnit: 'Единица возраста изменения', createdUnit: 'Единица возраста создания', noConditions: 'Без условий «Все условия» выбирает все файлы.', when: 'Когда запускать', schedule: 'Расписание', cron: 'Cron-выражение', cronHint: 'Минута, час, день, месяц, день недели. Используется местное время.', stabilityPeriod: 'Не трогать недавно изменённые файлы', stabilityHint: 'Даже при значении 0 файл должен быть стабилен минимум 2 секунды.', enableAfter: 'Включить правило после сохранения', save: 'Сохранить правило', saving: 'Сохранение…', closeEditor: 'Закрыть редактор', stop: 'Остановить', stopping: 'Остановка…', running: 'Выполняется', completed: 'Завершено', partial: 'С ошибками', failed: 'Ошибка', interrupted: 'Прервано', cancelled: 'Остановлено', disabledLabel: 'Выключено', calculating: 'Расчёт расписания…', noFuture: 'Нет будущих запусков', waiting: ' · ожидает завершения текущего запуска', allFiles: 'Все файлы', noMatching: 'Нет подходящих файлов', extension: 'Расширение', nameHas: 'Имя содержит', sizeFrom: 'Размер от', sizeTo: 'до', anySize: 'Любой размер', modifiedFor: 'Изменён не менее', createdFor: 'Создан не менее', ago: ' назад', seconds: 'сек.', minutes: 'мин.', hours: 'ч.', days: 'дн.', bytes: 'Б', kb: 'КБ', mb: 'МБ', gb: 'ГБ', tb: 'ТБ', five: 'Каждые 5 минут', hourly: 'Каждый час', daily: 'Каждый день в 00:00', custom: 'Своё расписание', exported: 'Правила экспортированы: {path}', imported: 'Добавлено правил: {count}. Они выключены — проверьте пути перед включением.', appOnly: 'Чтобы работать с файлами, откройте приложение Filemelon на компьютере.', nestedAst: 'Это правило содержит вложенные группы. Изменение такого AST пока доступно только через Rust/API.', anyNeeds: 'Для «любого условия» добавьте хотя бы одно условие.', enableRule: 'Включить правило', language: 'Язык', english: 'Английский', russian: 'Русский'
    }
};
type UiSnapshot = { rules: Rule[]; history: Run[]; next: NextRun[]; active: ActiveRun | null };
@Component({ selector: 'app-root', imports: [FormsModule], templateUrl: './app.component.html', styleUrl: './app.component.css' })
export class AppComponent implements OnInit, OnDestroy {
    language = signal<Language>(this.loadLanguage());
    private loadLanguage(): Language { try { return localStorage.getItem('filemelon.language') === 'ru' ? 'ru' : 'en'; } catch { return 'en'; } }
    setLanguage(value: string) { const language: Language = value === 'ru' ? 'ru' : 'en'; this.language.set(language); try { localStorage.setItem('filemelon.language', language); } catch {} }
    t(key: string) { return TEXT[this.language()][key] ?? key; }
    text(key: string, values: Record<string, string | number> = {}) { return Object.entries(values).reduce((result, [name, value]) => result.replace(`{${name}}`, String(value)), this.t(key)); }
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
        return ({ running: 'running', success: 'completed', partial: 'partial', failed: 'failed', interrupted: 'interrupted', cancelled: 'cancelled' } as Record<string, string>)[status] ? this.t(({ running: 'running', success: 'completed', partial: 'partial', failed: 'failed', interrupted: 'interrupted', cancelled: 'cancelled' } as Record<string, string>)[status]) : status;
    }
    conditionChips(condition: Condition) {
        return condition.type === 'all' || condition.type === 'any' ? condition.conditions.map(c => this.conditionLabel(c)) : [this.conditionLabel(condition)];
    }
    resetFilters() {
        this.search.set('');
        this.ruleFilter.set('all');
    }
    schedulePreset = 'hourly';
    schedulePresets = [{ value: 'five', cron: '*/5 * * * *' }, { value: 'hourly', cron: '0 * * * *' }, { value: 'daily', cron: '0 0 * * *' }, { value: 'custom', cron: '' }];
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
    timeUnits = [{ value: 1, key: 'seconds' }, { value: 60, key: 'minutes' }, { value: 3600, key: 'hours' }, { value: 86400, key: 'days' }];
    sizeUnits = [{ value: 1, key: 'bytes' }, { value: 1024, key: 'kb' }, { value: 1024 ** 2, key: 'mb' }, { value: 1024 ** 3, key: 'gb' }, { value: 1024 ** 4, key: 'tb' }];
    unitLabel(unit: { key: string }) { return this.t(unit.key); }
    minSizeUnit = 1024 ** 2;
    maxSizeUnit = 1024 ** 2;
    modifiedAgeUnit = 60;
    createdAgeUnit = 60;
    stabilityAge = 1;
    stabilityUnit = 86400;
    formatSize(bytes: number) {
        const unit = [...this.sizeUnits].reverse().find(u => bytes >= u.value) || this.sizeUnits[0];
        return `${(bytes / unit.value).toLocaleString(this.language() === 'ru' ? 'ru-RU' : 'en-US', { maximumFractionDigits: 3 })} ${this.t(unit.key)}`;
    }
    nextRunLabel(rule: Rule) {
        if (!rule.enabled)
            return this.t('disabledLabel');
        const next = this.nextRuns().get(rule.id!);
        if (!next)
            return this.t('calculating');
        if (next.error)
            return next.error;
        if (!next.time)
            return this.t('noFuture');
        return this.formatDate(next.time) + (next.waiting ? this.t('waiting') : '');
    }
    private displayUnit(value: number, units: {
        value: number;
        key: string;
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
            throw new Error(this.language() === 'ru' ? 'Укажите неотрицательное значение с точностью до байта или секунды.' : 'Enter a non-negative value with byte or second precision.');
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
                this.notice.set(this.text('exported', { path }));
        });
    }
    async importRules() {
        await this.act(async () => {
            const result = await invoke<{
                imported: number;
            } | null>('import_rules');
            if (result) {
                this.notice.set(this.text('imported', { count: result.imported }));
                await this.refresh();
            }
        });
    }
    pendingDelete = signal<number | null>(null);
    formatDate(value: string | null) {
        if (!value)
            return '—';
        const date = new Date(value);
        return Number.isNaN(date.getTime()) ? '—' : new Intl.DateTimeFormat(this.language() === 'ru' ? 'ru-RU' : 'en-US', { day: '2-digit', month: 'long', year: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit' }).format(date);
    }
    formatAge(seconds: number) {
        if (seconds === 0)
            return `0 ${this.t('seconds')}`;
        for (const [unit, key] of [[86400, 'days'], [3600, 'hours'], [60, 'minutes']] as const) {
            if (seconds % unit === 0)
                return `${seconds / unit} ${this.t(key)}`;
        }
        return `${seconds} ${this.t('seconds')}`;
    }
    conditionLabel(condition: Condition): string {
        switch (condition.type) {
            case 'all':
            case 'any': return condition.conditions.length ? condition.conditions.map(c => `(${this.conditionLabel(c)})`).join(condition.type === 'all' ? ` ${this.language() === 'ru' ? 'И' : 'AND'} ` : ` ${this.language() === 'ru' ? 'ИЛИ' : 'OR'} `) : this.t(condition.type === 'all' ? 'allFiles' : 'noMatching');
            case 'extension': return `${this.t('extension')}: ${condition.values.map(v => '.' + v.replace(/^\./, '')).join(', ')}`;
            case 'name_contains': return `${this.t('nameHas')} «${condition.value}»`;
            case 'size': return [condition.min !== null ? `${this.t('sizeFrom')} ${this.formatSize(condition.min)}` : '', condition.max !== null ? `${this.t('sizeTo')} ${this.formatSize(condition.max)}` : ''].filter(Boolean).join(' ') || this.t('anySize');
            case 'modified_age': return `${this.t('modifiedFor')} ${this.formatAge(condition.seconds)}${this.t('ago')}`;
            case 'created_age': return `${this.t('createdFor')} ${this.formatAge(condition.seconds)}${this.t('ago')}`;
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
        return { id: null, name: '', source: '', condition: { type: 'all', conditions: [] }, cron: '0 * * * *', enabled: true, action: 'MOVE', destination: '', min_age_seconds: 86400, include_subdirectories: false };
    }
    async ngOnInit() {
        if (!isTauri()) {
            this.error.set(this.t('appOnly'));
            return;
        }
        await this.act(async () => {
            await this.refresh();
            this.autostart.set(await invoke<boolean>('autostart_enabled'));
        });
        this.unlisten = await listen<UiSnapshot>('filemelon-state-changed', event => this.applySnapshot(event.payload));
        if (!this.error()) {
            // Report readiness only after Angular renders and the initial IPC calls succeed.
            requestAnimationFrame(() => requestAnimationFrame(() => { void invoke('startup_ready'); }));
        }
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
            this.error.set(this.t('nestedAst'));
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
                throw new Error(this.t('anyNeeds'));
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
