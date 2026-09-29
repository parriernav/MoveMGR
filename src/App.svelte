<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { cloneProject, emptyState, newProject } from './lib/defaults';
  import { projectFolderDialogOptions } from './lib/folder-dialog';
  import { languageOptions, localeForLanguage, reasonMessageKeys, translate, type Language, type MessageKey, type MessageVariables } from './lib/i18n';
  import { nextPlanSort, sortedPlanItems, type PlanSort, type PlanSortKey } from './lib/plan-sort';
  import { applyRuleTag, normalizedTagName, snapshotRules } from './lib/rule-tags';
  import { targetSummaryLines } from './lib/rule-summary';
  import * as api from './lib/ipc';
  import type { AppInfo, LocalState, Plan, Project, RuleTag, RunResult } from './lib/types';

  let state: LocalState = structuredClone(emptyState);
  let appInfo: AppInfo = { version: '0.0.1', platform: 'desktop', configDir: '' };
  let loading = true;
  let busy = false;
  let saved = true;
  let message = '';
  let error = '';
  let editing: Project | null = null;
  let extensionText = '';
  let activeTagId: string | null = null;
  let tagNameMode: 'create' | 'rename' | null = null;
  let tagNameDraft = '';
  let renamingId: string | null = null;
  let renameValue = '';
  let plan: Plan | null = null;
  let previewSort: PlanSort = { key: null, direction: 'asc' };
  let runResult: RunResult | null = null;
  let history: RunResult[] | null = null;
  let dialog: 'sourceRules' | 'targetRules' | 'plan' | 'history' | 'settings' | 'manual' | null = null;

  const t = (key: MessageKey, variables?: MessageVariables) => translate(state.preferences.language, key, variables);

  const checkedCount = () => state.projects.filter((project) => project.checked).length;
  const scopedProjects = () => {
    const checked = state.projects.filter((project) => project.checked);
    return checked.length ? checked : state.projects;
  };
  const projectIssues = (project: Project) => {
    const issues: string[] = [];
    if (!project.name.trim()) issues.push(t('projectName'));
    if (!project.source.root.trim()) issues.push(t('sourceFolder'));
    if (!project.target.root.trim()) issues.push(t('targetFolder'));
    if (project.source.extensions.mode === 'only' && project.source.extensions.values.length === 0) issues.push(t('extensions'));
    if (project.source.nameFilters.some((filter) => !filter.value.trim())) issues.push(t('filenameCondition'));
    const extractor = project.key.extractor;
    if (extractor.kind === 'firstChars' && (!Number.isInteger(extractor.count) || extractor.count < 1)) issues.push(t('keyLength'));
    if (extractor.kind === 'beforeDelimiter' && !extractor.delimiter) issues.push(t('keyDelimiter'));
    const destination = project.target.destination;
    if (destination.mode === 'fixedSubfolder' && !destination.relativePath.trim()) issues.push(t('targetSubfolder'));
    if (destination.mode === 'matchSubfolder') {
      if (destination.folderExtractor.kind === 'firstChars' && destination.folderExtractor.count < 1) issues.push(t('folderKeyLength'));
      if (destination.folderExtractor.kind === 'beforeDelimiter' && !destination.folderExtractor.delimiter) issues.push(t('folderDelimiter'));
      if (destination.comparison.kind === 'prefixEqual' && destination.comparison.count < 1) issues.push(t('compareLength'));
    }
    return issues;
  };
  const projectReady = (project: Project) => projectIssues(project).length === 0;
  const scopeReady = () => scopedProjects().length > 0 && scopedProjects().every(projectReady);
  const incompleteCount = () => scopedProjects().filter((project) => !projectReady(project)).length;
  const selectedIds = () => {
    const selected = state.projects.filter((project) => project.checked).map((project) => project.id);
    return selected.length ? selected : state.projects.map((project) => project.id);
  };

  onMount(async () => {
    try {
      if (api.isTauri()) {
        [state, appInfo] = await Promise.all([api.loadState(), api.getAppInfo()]);
      } else {
        const cached = localStorage.getItem('movemgr-preview-state');
        if (cached) state = JSON.parse(cached);
        message = t('browserPreview');
      }
      applyTheme();
    } catch (cause) {
      showError(cause);
    } finally {
      loading = false;
    }
  });

  function applyTheme() {
    document.documentElement.dataset.theme = state.preferences.theme;
    document.documentElement.lang = state.preferences.language;
  }

  function showError(cause: unknown) {
    error = typeof cause === 'string' ? cause : cause instanceof Error ? cause.message : JSON.stringify(cause);
    message = '';
  }

  async function persist(next: LocalState) {
    saved = false;
    error = '';
    try {
      if (api.isTauri()) {
        state = await api.saveState(next, state.revision);
      } else {
        state = { ...next, revision: state.revision + 1 };
        localStorage.setItem('movemgr-preview-state', JSON.stringify(state));
      }
      saved = true;
      applyTheme();
    } catch (cause) {
      showError(cause);
      throw cause;
    }
  }

  async function updateChecks(mode: 'all' | 'none') {
    if (busy) return;
    const next = structuredClone(state);
    next.projects.forEach((project) => (project.checked = mode === 'all'));
    await persist(next);
  }

  async function toggleProject(id: string, checked: boolean) {
    const next = structuredClone(state);
    const project = next.projects.find((item) => item.id === id);
    if (project) project.checked = checked;
    await persist(next);
  }

  async function addProject() {
    const project = newProject(state.preferences.language);
    const next = structuredClone(state);
    next.projects.push(project);
    await persist(next);
    renamingId = project.id;
    renameValue = project.name;
    message = t('projectAdded');
  }

  async function startRename(project: Project) {
    if (busy) return;
    renamingId = project.id;
    renameValue = project.name;
    await tick();
    document.getElementById(`project-name-${project.id}`)?.focus();
  }

  async function commitRename(id: string) {
    if (renamingId !== id) return;
    const name = renameValue.trim();
    if (!name) {
      error = t('projectNameRequired');
      return;
    }
    renamingId = null;
    const next = structuredClone(state);
    const project = next.projects.find((item) => item.id === id);
    if (project && project.name !== name) {
      project.name = name;
      await persist(next);
    }
  }

  async function chooseProjectFolder(project: Project, side: 'source' | 'target') {
    if (!api.isTauri()) return showError(t('desktopFolderOnly'));
    const selected = await open(projectFolderDialogOptions(project, side));
    if (typeof selected === 'string') {
      const next = structuredClone(state);
      const changed = next.projects.find((item) => item.id === project.id);
      if (!changed) return;
      if (side === 'source') changed.source.root = selected;
      else changed.target.root = selected;
      await persist(next);
      message = t('folderChanged', { side: t(side) });
    }
  }

  function openRules(project: Project, kind: 'sourceRules' | 'targetRules') {
    editing = structuredClone(project);
    extensionText = editing.source.extensions.mode === 'only' ? editing.source.extensions.values.join(', ') : '';
    activeTagId = null;
    tagNameMode = null;
    tagNameDraft = '';
    dialog = kind;
  }

  const currentTagKind = (): RuleTag['kind'] | null => dialog === 'sourceRules' ? 'source' : dialog === 'targetRules' ? 'target' : null;
  const tagsFor = (kind: RuleTag['kind']) => (state.ruleTags ?? []).filter((tag) => tag.kind === kind);

  function normalizedEditing(): Project | null {
    if (!editing) return null;
    const draft = structuredClone(editing);
    if (draft.source.extensions.mode === 'only') {
      draft.source.extensions.values = [...new Set(extensionText.split(',').map((value) => value.trim().replace(/^\*?\./, '').toLowerCase()).filter(Boolean))];
    }
    draft.source.nameFilters = draft.source.nameFilters.map((filter) => ({ ...filter, value: filter.value.trim() }));
    const validation = validateRules(draft);
    if (validation) {
      error = validation;
      return null;
    }
    return draft;
  }

  function beginTagName(mode: 'create' | 'rename') {
    const selected = (state.ruleTags ?? []).find((tag) => tag.id === activeTagId && tag.kind === currentTagKind());
    if (mode === 'rename' && !selected) return;
    tagNameMode = mode;
    tagNameDraft = mode === 'rename' ? selected?.name ?? '' : '';
    tick().then(() => document.getElementById('rule-tag-name')?.focus());
  }

  async function submitTagName() {
    const kind = currentTagKind();
    if (!kind || !tagNameMode) return;
    const name = normalizedTagName(tagNameDraft);
    if (!name) return showError(t('tagNameRequired'));
    if ([...name].length > 80) return showError(t('tagNameTooLong'));
    const selectedId = tagNameMode === 'rename' ? activeTagId : null;
    if ((state.ruleTags ?? []).some((tag) => tag.kind === kind && tag.id !== selectedId && tag.name.toLocaleLowerCase() === name.toLocaleLowerCase())) {
      return showError(t('duplicateTagName'));
    }
    const next = structuredClone(state);
    next.ruleTags ??= [];
    if (tagNameMode === 'create') {
      const draft = normalizedEditing();
      if (!draft) return;
      const id = crypto.randomUUID();
      if (kind === 'source') next.ruleTags.push({ id, name, kind, rules: snapshotRules(draft, 'source') });
      else next.ruleTags.push({ id, name, kind, rules: snapshotRules(draft, 'target') });
      try { await persist(next); } catch { return; }
      activeTagId = id;
      message = t('tagSaved', { name });
    } else {
      const tag = next.ruleTags.find((item) => item.id === selectedId && item.kind === kind);
      if (!tag) return;
      tag.name = name;
      try { await persist(next); } catch { return; }
      message = t('tagRenamed', { name });
    }
    tagNameMode = null;
    tagNameDraft = '';
  }

  async function applyTag(tag: RuleTag) {
    if (!editing || tag.kind !== currentTagKind()) return;
    const changed = applyRuleTag(editing, tag);
    const next = structuredClone(state);
    const index = next.projects.findIndex((project) => project.id === changed.id);
    if (index < 0) return;
    next.projects[index] = changed;
    try { await persist(next); } catch { return; }
    editing = structuredClone(changed);
    extensionText = editing.source.extensions.mode === 'only' ? editing.source.extensions.values.join(', ') : '';
    activeTagId = tag.id;
    tagNameMode = null;
    message = t('tagApplied', { name: tag.name });
  }

  async function updateTag() {
    const kind = currentTagKind();
    const draft = normalizedEditing();
    if (!kind || !draft || !activeTagId) return;
    const next = structuredClone(state);
    const index = (next.ruleTags ?? []).findIndex((tag) => tag.id === activeTagId && tag.kind === kind);
    if (index < 0) return;
    const tag = next.ruleTags[index];
    next.ruleTags[index] = kind === 'source'
      ? { id: tag.id, name: tag.name, kind, rules: snapshotRules(draft, 'source') }
      : { id: tag.id, name: tag.name, kind, rules: snapshotRules(draft, 'target') };
    try { await persist(next); } catch { return; }
    message = t('tagUpdated', { name: tag.name });
  }

  async function deleteTag() {
    const kind = currentTagKind();
    const tag = (state.ruleTags ?? []).find((item) => item.id === activeTagId && item.kind === kind);
    if (!tag || !confirm(t('deleteTagConfirm', { name: tag.name }))) return;
    const next = structuredClone(state);
    next.ruleTags = next.ruleTags.filter((item) => item.id !== tag.id);
    try { await persist(next); } catch { return; }
    activeTagId = null;
    tagNameMode = null;
    message = t('tagDeleted', { name: tag.name });
  }

  function validateRules(project: Project) {
    if (project.source.extensions.mode === 'only' && project.source.extensions.values.length === 0) return t('extensionRequired');
    if (project.source.nameFilters.some((filter) => !filter.value.trim())) return t('filterTextRequired');
    const extractor = project.key.extractor;
    if (extractor.kind === 'firstChars' && (!Number.isInteger(extractor.count) || extractor.count < 1)) return t('keyLengthInvalid');
    if (extractor.kind === 'beforeDelimiter' && !extractor.delimiter) return t('delimiterRequired');
    const destination = project.target.destination;
    if (destination.mode === 'matchSubfolder' && destination.comparison.kind === 'prefixEqual' && destination.comparison.count < 1) return t('compareLengthInvalid');
    return null;
  }

  async function saveRules() {
    const draft = normalizedEditing();
    if (!draft) return;
    const next = structuredClone(state);
    const index = next.projects.findIndex((item) => item.id === draft.id);
    if (index < 0) return;
    next.projects[index] = draft;
    await persist(next);
    dialog = null;
    message = t('rulesSaved');
  }

  async function duplicate(project: Project) {
    const next = structuredClone(state);
    next.projects.push(cloneProject(project, state.preferences.language));
    await persist(next);
  }

  async function removeProject(project: Project) {
    if (!confirm(t('deleteProjectConfirm', { name: project.name }))) return;
    const next = structuredClone(state);
    next.projects = next.projects.filter((item) => item.id !== project.id);
    await persist(next);
  }

  async function moveProject(index: number, direction: -1 | 1) {
    const target = index + direction;
    if (target < 0 || target >= state.projects.length) return;
    const next = structuredClone(state);
    [next.projects[index], next.projects[target]] = [next.projects[target], next.projects[index]];
    await persist(next);
  }

  function sourceSummary(project: Project) {
    const extension = project.source.extensions.mode === 'all' ? t('allFiles') : project.source.extensions.values.map((item) => item.toUpperCase()).join(' · ');
    const recursion = project.source.recursive ? t('recursive') : t('currentFolder');
    return `${extension || t('extensionsUnset')} · ${recursion}`;
  }

  async function preparePlan(ids = selectedIds(), executeImmediately = false) {
    if (!ids.length || busy) return;
    if (!api.isTauri()) {
      error = t('desktopMoveOnly');
      return;
    }
    busy = true;
    error = '';
    message = t('scanning');
    try {
      plan = await api.createPlan(ids);
      previewSort = { key: null, direction: 'asc' };
      runResult = null;
      if (executeImmediately && !state.preferences.previewBeforeRun) await executePlan();
      else dialog = 'plan';
      message = '';
    } catch (cause) {
      showError(cause);
    } finally {
      busy = false;
    }
  }

  async function executePlan() {
    if (!plan || busy) return;
    busy = true;
    error = '';
    message = t('moving');
    try {
      runResult = await api.startRun(plan.planId);
      plan = null;
      dialog = 'plan';
      message = runResult.failed || runResult.sourceRetained ? t('reviewItems') : t('moveComplete');
    } catch (cause) {
      showError(cause);
    } finally {
      busy = false;
    }
  }

  async function cancelRun() {
    try {
      await api.cancelRun();
      message = t('cancelling');
    } catch (cause) {
      showError(cause);
    }
  }

  async function openHistory() {
    if (!api.isTauri()) return showError(t('desktopHistoryOnly'));
    try {
      history = await api.listHistory();
      dialog = 'history';
    } catch (cause) {
      showError(cause);
    }
  }

  async function exportConfig() {
    if (!api.isTauri()) return showError(t('desktopExportOnly'));
    const path = await save({ title: t('exportTitle'), defaultPath: 'movemgr-settings.json', filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (!path) return;
    await api.exportSettings(path);
    message = t('exported');
  }

  async function importConfig() {
    if (!api.isTauri()) return showError(t('desktopImportOnly'));
    const path = await open({ title: t('importTitle'), multiple: false, filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (typeof path !== 'string') return;
    const append = confirm(t('importModeConfirm'));
    try {
      state = await api.importSettings(path, append ? 'append' : 'replace');
      applyTheme();
      message = append ? t('projectsAppended') : t('settingsReplaced');
    } catch (cause) {
      showError(cause);
    }
  }

  async function changePreference<K extends keyof LocalState['preferences']>(key: K, value: LocalState['preferences'][K]) {
    const next = structuredClone(state);
    next.preferences[key] = value;
    await persist(next);
    if (key === 'language') {
      error = '';
      message = api.isTauri() ? '' : translate(value as Language, 'browserPreview');
    }
  }

  function reasonText(code: string | null, fallback: string | null) {
    const key = code ? reasonMessageKeys[code] : undefined;
    return key ? t(key) : fallback;
  }
</script>

<svelte:head><title>MoveMgr ver.{appInfo.version}</title></svelte:head>

{#snippet tagShelf(kind: 'source' | 'target')}
  <section class="tag-shelf" aria-label={t(kind === 'source' ? 'sourceRuleTags' : 'targetRuleTags')}>
    <div class="tag-shelf-top">
      <span class="tag-shelf-title"><span aria-hidden="true">🏷️</span> {t(kind === 'source' ? 'sourceTags' : 'targetTags')}</span>
      <div class="tag-actions">
        <button class="tag-icon" title={t('saveNewTag')} aria-label={t('saveNewTag')} onclick={() => beginTagName('create')}><span aria-hidden="true">✚</span></button>
        <button class="tag-icon" title={t('updateTag')} aria-label={t('updateTag')} disabled={!activeTagId} onclick={updateTag}><span aria-hidden="true">💾</span></button>
        <button class="tag-icon" title={t('renameTag')} aria-label={t('renameTag')} disabled={!activeTagId} onclick={() => beginTagName('rename')}><span aria-hidden="true">✎</span></button>
        <button class="tag-icon tag-delete" title={t('deleteTag')} aria-label={t('deleteTag')} disabled={!activeTagId} onclick={deleteTag}><span aria-hidden="true">🗑</span></button>
      </div>
    </div>
    <div class="tag-list">
      {#each tagsFor(kind) as tag (tag.id)}
        <button class:selected={activeTagId === tag.id} class="tag-chip" title={t('applyTag', { name: tag.name })} aria-label={t('applyTagAria', { name: tag.name })} aria-pressed={activeTagId === tag.id} onclick={() => applyTag(tag)}><span aria-hidden="true">✦</span>{tag.name}</button>
      {:else}
        <span class="tag-empty">{t('noTags')}</span>
      {/each}
    </div>
    {#if tagNameMode}
      <div class="tag-name-entry">
        <input id="rule-tag-name" bind:value={tagNameDraft} maxlength="80" placeholder={t('tagName')} aria-label={t('tagName')} onkeydown={(event) => { if (event.key === 'Enter') { event.preventDefault(); submitTagName(); } else if (event.key === 'Escape') tagNameMode = null; }} />
        <button class="tag-icon tag-confirm" title={t('saveName')} aria-label={t('saveName')} onclick={submitTagName}><span aria-hidden="true">✓</span></button>
        <button class="tag-icon" title={t('cancel')} aria-label={t('cancelName')} onclick={() => (tagNameMode = null)}><span aria-hidden="true">×</span></button>
      </div>
    {/if}
  </section>
{/snippet}

{#if loading}
  <main class="loading"><div class="app-mark">M</div><p>{t('loading')}</p></main>
{:else}
  <div class="app-shell">
    <header class="topbar">
      <div class="brand">
        <img src="/icon.svg" alt="" />
        <div><strong>MoveMgr</strong><span>ver.{appInfo.version}</span></div>
      </div>
      <nav aria-label={t('appMenu')}>
        <label class="language-picker"><span>{t('language')}</span><select aria-label={t('language')} value={state.preferences.language} onchange={(event) => changePreference('language', event.currentTarget.value as Language)}>{#each languageOptions as option}<option value={option.value}>{option.label}</option>{/each}</select></label>
        <button class="quiet" onclick={() => (dialog = 'manual')}>{t('manual')}</button>
        <button class="quiet" onclick={openHistory}>{t('history')}</button>
        <button class="quiet" onclick={importConfig} disabled={busy}>{t('import')}</button>
        <button class="quiet" onclick={exportConfig}>{t('export')}</button>
        <button class="quiet" onclick={() => (dialog = 'settings')}>{t('settings')}</button>
      </nav>
    </header>

    <main class="workspace">
      <section class="intro">
        <div><span class="eyebrow">{t('workspace')}</span><h1>{t('heroTitle')}</h1><p>{t('heroBody')}</p></div>
        <button class="primary" onclick={addProject} disabled={busy}><span aria-hidden="true">＋</span> {t('addProject')}</button>
      </section>

      <section class="toolbar" aria-label={t('projectTools')}>
        <div class="selection-tools">
          <button class="quiet" onclick={() => updateChecks('all')} disabled={busy || !state.projects.length}>{t('selectAll')}</button>
          <button class="quiet" onclick={() => updateChecks('none')} disabled={busy || !state.projects.length}>{t('clearAll')}</button>
          <span>{t('selectionCount', { selected: checkedCount(), total: state.projects.length })}{#if incompleteCount()}{t('setupNeededCount', { count: incompleteCount() })}{/if}</span>
        </div>
        <div class="toolbar-actions">
          <button onclick={() => preparePlan()} disabled={busy || !scopeReady()} title={scopeReady() ? t('previewScope') : t('finishRequired')}>{t('preview')}</button>
          <button class="primary" onclick={() => preparePlan(selectedIds(), true)} disabled={busy || !scopeReady()} title={scopeReady() ? t('runScope') : t('finishRequired')}>
            <span aria-hidden="true">▶</span> {checkedCount() ? t('runSome', { count: checkedCount() }) : t('runAll', { count: state.projects.length })}
          </button>
        </div>
      </section>

      <section class="project-list" aria-label={t('projectList')}>
          <div class="board-head" aria-hidden="true">
            <span>{t('selectRun')}</span><span>{t('projectName')}</span><span>{t('sourceFolder')}</span><span>{t('sourceRules')}</span><span>{t('targetFolder')}</span><span>{t('targetRules')}</span><span>{t('management')}</span>
          </div>
          {#if !state.projects.length}
            <div class="board-empty"><span>{t('noProjects')}</span><button onclick={addProject} disabled={busy}>＋ {t('addFirstRow')}</button></div>
          {/if}
          {#each state.projects as project, index (project.id)}
            <article class:checked={project.checked} class:incomplete={!projectReady(project)} class="project-row">
              <div class="run-cell">
                <input class="check" type="checkbox" aria-label={t('selectProject', { name: project.name })} checked={project.checked} disabled={busy} onchange={(event) => toggleProject(project.id, event.currentTarget.checked)} />
                <button class="play" aria-label={t('runOneAria', { name: project.name })} title={projectReady(project) ? t('runThisProject') : `${t('setupNeeded')}: ${projectIssues(project).join(', ')}`} disabled={busy || !projectReady(project)} onclick={() => preparePlan([project.id], true)}>▶</button>
              </div>
              <div class="name-cell">
                {#if renamingId === project.id}
                  <input id={`project-name-${project.id}`} class="inline-name-input" bind:value={renameValue} maxlength="80" aria-label={t('projectNameAria')} onblur={() => commitRename(project.id)} onkeydown={(event) => { if (event.key === 'Enter') { event.preventDefault(); commitRename(project.id); } else if (event.key === 'Escape') renamingId = null; }} />
                {:else}
                  <button class="board-cell-button name-button" title={t('renameProject')} onclick={() => startRename(project)}><strong>{project.name}</strong><span>{t('clickToRename')}</span></button>
                {/if}
                {#if !projectReady(project)}<small class="required">{t('setupNeeded')}</small>{/if}
              </div>
              <button class:missing={!project.source.root} class="board-cell-button" title={project.source.root || t('chooseSourceFolder')} disabled={busy} onclick={() => chooseProjectFolder(project, 'source')}><strong>{project.source.root ? t('sourceFolder') : t('notSet')}</strong><span>{project.source.root || t('clickToChoose')}</span></button>
              <button class="board-cell-button" title={t('editSourceRules')} disabled={busy} onclick={() => openRules(project, 'sourceRules')}><strong>{t('sourceRules')}</strong><span>{sourceSummary(project)}</span></button>
              <button class:missing={!project.target.root} class="board-cell-button" title={project.target.root || t('chooseTargetFolder')} disabled={busy} onclick={() => chooseProjectFolder(project, 'target')}><strong>{project.target.root ? t('targetFolder') : t('notSet')}</strong><span>{project.target.root || t('clickToChoose')}</span></button>
              <button class="board-cell-button target-rules-button" title={targetSummaryLines(project, state.preferences.language).join('\n')} disabled={busy} onclick={() => openRules(project, 'targetRules')}><strong>{t('targetRules')}</strong>{#each targetSummaryLines(project, state.preferences.language) as line}<span>{line}</span>{/each}</button>
              <div class="row-actions" aria-label={t('projectManagement', { name: project.name })}>
                <button class="icon-button" aria-label={t('moveUp')} title={t('moveUp')} disabled={busy || index === 0} onclick={() => moveProject(index, -1)}>↑</button>
                <button class="icon-button" aria-label={t('moveDown')} title={t('moveDown')} disabled={busy || index === state.projects.length - 1} onclick={() => moveProject(index, 1)}>↓</button>
                <button class="icon-button" aria-label={t('duplicateProject')} title={t('duplicate')} disabled={busy} onclick={() => duplicate(project)}>⧉</button>
                <button class="icon-button danger-text" aria-label={t('deleteProject')} title={t('delete')} disabled={busy} onclick={() => removeProject(project)}>×</button>
              </div>
            </article>
          {/each}
      </section>

      <p class="selection-hint">{t('selectionHint')}</p>
      <footer class="statusbar"><span>{busy ? t('working') : saved ? t('settingsSaved') : t('saving')}</span><span>{state.preferences.previewBeforeRun ? t('previewOn') : t('quickRunOn')} · {t('rememberChecks')}</span></footer>
      {#if message}<div class="toast success" role="status">{message}</div>{/if}
      {#if error}<div class="toast error" role="alert">{error}<button aria-label={t('closeError')} onclick={() => (error = '')}>×</button></div>{/if}
    </main>
  </div>
{/if}

{#if dialog === 'sourceRules' && editing}
  <div class="modal-backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && (dialog = null)}>
    <div class="modal rules-modal" role="dialog" aria-modal="true" aria-labelledby="source-rules-title">
      <header class="modal-header"><div><span class="eyebrow">SOURCE RULES</span><h2 id="source-rules-title">{editing.name} · {t('sourceRules')}</h2><p>{t('sourceRulesHelp')}</p></div><button class="close" aria-label={t('close')} onclick={() => (dialog = null)}>×</button></header>
      {@render tagShelf('source')}
      <div class="rules-body">
        <fieldset><legend>{t('extensions')}</legend>
          <div class="choice-row"><label><input type="radio" name="extMode" checked={editing.source.extensions.mode === 'all'} onchange={() => { if (editing) editing.source.extensions = { mode: 'all' }; }} /> {t('allFiles')}</label><label><input type="radio" name="extMode" checked={editing.source.extensions.mode === 'only'} onchange={() => { if (editing) editing.source.extensions = { mode: 'only', values: [], includeExtensionless: false }; }} /> {t('selectedExtensions')}</label></div>
          {#if editing.source.extensions.mode === 'only'}<label class="field"><span>{t('commaSeparated')}</span><input bind:value={extensionText} placeholder="jpg, png, pdf" /></label>{/if}
          <div class="choice-row"><label><input type="checkbox" bind:checked={editing.source.recursive} /> {t('recursive')}</label><label><input type="checkbox" bind:checked={editing.source.includeHidden} /> {t('includeHidden')}</label></div>
        </fieldset>
        <fieldset><legend>{t('filenameConditions')}</legend>
          <p class="field-help">{t('conditionsHelp')}</p>
          <div class="filter-list">
            {#each editing.source.nameFilters as filter, i}
              <div class="filter-row"><select bind:value={filter.op}><option value="startsWith">{t('startsWith')}</option><option value="contains">{t('contains')}</option><option value="endsWith">{t('endsWith')}</option><option value="equals">{t('equals')}</option></select><input bind:value={filter.value} placeholder={t('text')} /><button aria-label={t('removeCondition')} onclick={() => { editing?.source.nameFilters.splice(i, 1); editing = structuredClone(editing); }}>×</button></div>
            {/each}
            <button class="text-button" onclick={() => { editing?.source.nameFilters.push({ op: 'startsWith', value: '' }); editing = structuredClone(editing); }}>＋ {t('addCondition')}</button>
          </div>
        </fieldset>
      </div>
      <footer class="modal-actions"><button onclick={() => (dialog = null)}>{t('cancel')}</button><button class="primary" onclick={saveRules}>{t('saveSourceRules')}</button></footer>
    </div>
  </div>
{/if}

{#if dialog === 'targetRules' && editing}
  <div class="modal-backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && (dialog = null)}>
    <div class="modal rules-modal" role="dialog" aria-modal="true" aria-labelledby="target-rules-title">
      <header class="modal-header"><div><span class="eyebrow">TARGET RULES</span><h2 id="target-rules-title">{editing.name} · {t('targetRules')}</h2><p>{t('targetRulesHelp')}</p></div><button class="close" aria-label={t('close')} onclick={() => (dialog = null)}>×</button></header>
      {@render tagShelf('target')}
      <div class="rules-body two-columns">
        <fieldset><legend>{t('extractKey')}</legend>
          <label class="field"><span>{t('extractionMethod')}</span><select value={editing.key.extractor.kind} onchange={(event) => { const kind = event.currentTarget.value; if (kind === 'whole') editing!.key.extractor = { kind: 'whole' }; else if (kind === 'firstChars') editing!.key.extractor = { kind: 'firstChars', count: 3 }; else editing!.key.extractor = { kind: 'beforeDelimiter', delimiter: '_', missing: 'skip' }; editing = structuredClone(editing); }}><option value="whole">{t('wholeName')}</option><option value="firstChars">{t('firstNChars')}</option><option value="beforeDelimiter">{t('beforeDelimiter')}</option></select></label>
          {#if editing.key.extractor.kind === 'firstChars'}<label class="field"><span>{t('characterCount')}</span><input type="number" min="1" max="255" bind:value={editing.key.extractor.count} /></label>{/if}
          {#if editing.key.extractor.kind === 'beforeDelimiter'}<label class="field"><span>{t('delimiter')}</span><input bind:value={editing.key.extractor.delimiter} maxlength="32" /></label><label class="field"><span>{t('whenMissingDelimiter')}</span><select bind:value={editing.key.extractor.missing}><option value="skip">{t('skip')}</option><option value="useWhole">{t('useWholeName')}</option></select></label>{/if}
          <label class="inline-check"><input type="checkbox" bind:checked={editing.comparisonOptions.ignoreCase} /> {t('ignoreCase')}</label>
        </fieldset>

        <fieldset><legend>{t('destination')}</legend>
          <label class="field"><span>{t('destinationMethod')}</span><select value={editing.target.destination.mode} onchange={(event) => { const mode = event.currentTarget.value; if (mode === 'root') editing!.target.destination = { mode: 'root' }; else if (mode === 'fixedSubfolder') editing!.target.destination = { mode, relativePath: '', createIfMissing: true }; else if (mode === 'keySubfolder') editing!.target.destination = { mode, parentRelativePath: '', createIfMissing: true }; else editing!.target.destination = { mode: 'matchSubfolder', searchBase: '', folderExtractor: { kind: 'whole' }, comparison: { kind: 'equals' }, noMatch: 'skip' }; editing = structuredClone(editing); }}><option value="root">{t('targetRootDirect')}</option><option value="fixedSubfolder">{t('fixedSubfolder')}</option><option value="matchSubfolder">{t('matchSubfolder')}</option><option value="keySubfolder">{t('keySubfolder')}</option></select></label>
          {#if editing.target.destination.mode === 'fixedSubfolder'}<label class="field"><span>{t('subfolder')}</span><input bind:value={editing.target.destination.relativePath} placeholder={t('completedPlaceholder')} /></label><label class="inline-check"><input type="checkbox" bind:checked={editing.target.destination.createIfMissing} /> {t('createIfMissing')}</label>{/if}
          {#if editing.target.destination.mode === 'keySubfolder'}<label class="field"><span>{t('baseSubfolder')}</span><input bind:value={editing.target.destination.parentRelativePath} /></label><label class="inline-check"><input type="checkbox" bind:checked={editing.target.destination.createIfMissing} /> {t('createIfMissing')}</label>{/if}
          {#if editing.target.destination.mode === 'matchSubfolder'}
            <label class="field"><span>{t('searchBase')}</span><input bind:value={editing.target.destination.searchBase} placeholder={t('emptyMeansRoot')} /></label>
            <label class="field"><span>{t('extractFolderKey')}</span><select value={editing.target.destination.folderExtractor.kind} onchange={(event) => { const kind = event.currentTarget.value; if (editing?.target.destination.mode !== 'matchSubfolder') return; if (kind === 'whole') editing.target.destination.folderExtractor = { kind: 'whole' }; else if (kind === 'firstChars') editing.target.destination.folderExtractor = { kind: 'firstChars', count: 3 }; else editing.target.destination.folderExtractor = { kind: 'beforeDelimiter', delimiter: '_', missing: 'skip' }; editing = structuredClone(editing); }}><option value="whole">{t('wholeFolderName')}</option><option value="firstChars">{t('folderFirstNChars')}</option><option value="beforeDelimiter">{t('folderBeforeDelimiter')}</option></select></label>
            {#if editing.target.destination.folderExtractor.kind === 'firstChars'}<label class="field"><span>{t('folderExtractLength')}</span><input type="number" min="1" max="255" bind:value={editing.target.destination.folderExtractor.count} /></label>{/if}
            {#if editing.target.destination.folderExtractor.kind === 'beforeDelimiter'}<label class="field"><span>{t('folderDelimiter')}</span><input bind:value={editing.target.destination.folderExtractor.delimiter} maxlength="32" /></label>{/if}
            <label class="field"><span>{t('comparisonMethod')}</span><select value={editing.target.destination.comparison.kind} onchange={(event) => { const kind = event.currentTarget.value; editing!.target.destination.mode === 'matchSubfolder' && (editing!.target.destination.comparison = kind === 'prefixEqual' ? { kind, count: 3 } : { kind } as {kind:'equals'|'startsWith'}); editing = structuredClone(editing); }}><option value="equals">{t('keyEquals')}</option><option value="startsWith">{t('folderStartsWithKey')}</option><option value="prefixEqual">{t('prefixesEqual')}</option></select></label>
            {#if editing.target.destination.comparison.kind === 'prefixEqual'}<label class="field"><span>{t('comparisonLength')}</span><input type="number" min="1" max="255" bind:value={editing.target.destination.comparison.count} /></label>{/if}
            <label class="field"><span>{t('whenNoMatch')}</span><select bind:value={editing.target.destination.noMatch}><option value="skip">{t('skip')}</option><option value="createKeyFolder">{t('createKeyFolder')}</option></select></label>
          {/if}
          <label class="field"><span>{t('nameConflict')}</span><select bind:value={editing.conflict}><option value="skip">{t('skip')}</option><option value="renameWithNumber">{t('renameWithNumber')}</option></select></label>
        </fieldset>
      </div>
      <footer class="modal-actions"><button onclick={() => (dialog = null)}>{t('cancel')}</button><button class="primary" onclick={saveRules}>{t('saveTargetRules')}</button></footer>
    </div>
  </div>
{/if}

{#if dialog === 'plan'}
  <div class="modal-backdrop"><div class="modal plan-modal" role="dialog" aria-modal="true" aria-labelledby="plan-title">
    <header class="modal-header"><div><span class="eyebrow">MOVE PREVIEW</span><h2 id="plan-title">{runResult ? t('moveResult') : t('reviewBeforeMove')}</h2></div><button class="close" aria-label={t('close')} onclick={() => (dialog = null)}>×</button></header>
    {#if runResult}
      <div class="metrics"><div><strong>{runResult.moved}</strong><span>{t('moved')}</span></div><div><strong>{runResult.skipped}</strong><span>{t('skipped')}</span></div><div><strong>{runResult.failed}</strong><span>{t('failed')}</span></div><div><strong>{runResult.sourceRetained}</strong><span>{t('sourceRetained')}</span></div></div>
      <div class="table-scroll"><table><thead><tr><th>{t('project')}</th><th>{t('original')}</th><th>{t('finalLocation')}</th><th>{t('status')}</th></tr></thead><tbody>{#each runResult.items as item}<tr><td>{item.projectName}</td><td>{item.sourcePath}</td><td>{item.finalTargetPath ?? '—'}</td><td>{item.state}{#if item.message}<small>{item.message}</small>{/if}</td></tr>{/each}</tbody></table></div>
    {:else if plan}
      <div class="metrics"><div><strong>{plan.movable}</strong><span>{t('scheduled')}</span></div><div><strong>{plan.skipped}</strong><span>{t('skipped')}</span></div><div><strong>{plan.blocked}</strong><span>{t('blocked')}</span></div></div>
      <div class="table-scroll"><table class="preview-table"><thead><tr>
        {#each [
          { key: 'project', label: t('project') },
          { key: 'source', label: t('originalFile') },
          { key: 'key', label: t('key') },
          { key: 'target', label: t('destination') },
          { key: 'decision', label: t('decision') }
        ] as column}
          <th aria-sort={previewSort.key === column.key ? (previewSort.direction === 'asc' ? 'ascending' : 'descending') : 'none'}>
            <button class="sort-heading" title={`${column.label} ${previewSort.key === column.key && previewSort.direction === 'asc' ? t('descending') : t('ascending')} ${t('sort')}`} onclick={() => (previewSort = nextPlanSort(previewSort, column.key as PlanSortKey))}>
              {column.label}<span class:active={previewSort.key === column.key} aria-hidden="true">{previewSort.key === column.key ? (previewSort.direction === 'asc' ? '▲' : '▼') : '↕'}</span>
            </button>
          </th>
        {/each}
      </tr></thead><tbody>{#each sortedPlanItems(plan.items, previewSort) as item (item.itemId)}<tr><td>{item.projectName}</td><td>{item.sourcePath}</td><td>{item.extractedKey ?? '—'}</td><td>{item.proposedTargetPath ?? '—'}</td><td><span class:ok={item.decision === 'move'} class:warn={item.decision !== 'move'}>{item.decision === 'move' ? t('move') : reasonText(item.reasonCode, item.reasonText) ?? item.decision}</span></td></tr>{/each}</tbody></table></div>
    {/if}
    <footer class="modal-actions"><button onclick={() => (dialog = null)} disabled={busy}>{t('close')}</button>{#if plan && !runResult}{#if busy}<button class="danger-button" onclick={cancelRun}>{t('cancelRun')}</button>{/if}<button class="primary" disabled={busy || plan.movable === 0 || plan.blocked > 0} onclick={executePlan}>{busy ? t('movingShort') : t('startMove', { count: plan.movable })}</button>{/if}</footer>
  </div></div>
{/if}

{#if dialog === 'history'}
  <div class="modal-backdrop"><div class="modal history-modal" role="dialog" aria-modal="true" aria-labelledby="history-title"><header class="modal-header"><div><span class="eyebrow">RUN HISTORY</span><h2 id="history-title">{t('history')}</h2></div><button class="close" aria-label={t('close')} onclick={() => (dialog = null)}>×</button></header>{#if history?.length}<div class="history-list">{#each history as item}<article><div><strong>{new Date(item.finishedAt).toLocaleString(localeForLanguage[state.preferences.language])}</strong><small>{item.runId}</small></div><div>{t('historySummary', { moved: item.moved, skipped: item.skipped, failed: item.failed, retained: item.sourceRetained })}</div></article>{/each}</div>{:else}<div class="empty-compact">{t('noHistory')}</div>{/if}<footer class="modal-actions"><button onclick={() => (dialog = null)}>{t('close')}</button></footer></div></div>
{/if}

{#if dialog === 'settings'}
  <div class="modal-backdrop"><div class="modal settings-modal" role="dialog" aria-modal="true" aria-labelledby="settings-title"><header class="modal-header"><div><span class="eyebrow">{t('preferences')}</span><h2 id="settings-title">{t('settings')}</h2></div><button class="close" aria-label={t('close')} onclick={() => (dialog = null)}>×</button></header><div class="settings-body"><label class="field"><span>{t('theme')}</span><select value={state.preferences.theme} onchange={(event) => changePreference('theme', event.currentTarget.value as LocalState['preferences']['theme'])}><option value="system">{t('themeSystem')}</option><option value="light">{t('themeLight')}</option><option value="dark">{t('themeDark')}</option></select></label><label class="switch-row"><span><strong>{t('previewBeforeRun')}</strong><small>{t('previewDescription')}</small></span><input type="checkbox" checked={state.preferences.previewBeforeRun} onchange={(event) => changePreference('previewBeforeRun', event.currentTarget.checked)} /></label><div class="about"><strong>MoveMgr ver.{appInfo.version}</strong><span>{appInfo.platform}</span>{#if appInfo.configDir}<small>{t('configLocation', { path: appInfo.configDir })}</small>{/if}</div></div><footer class="modal-actions"><button onclick={() => (dialog = null)}>{t('close')}</button></footer></div></div>
{/if}

{#if dialog === 'manual'}
  <div class="modal-backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && (dialog = null)}><div class="modal manual-modal" role="dialog" aria-modal="true" aria-labelledby="manual-title"><header class="modal-header"><div><span class="eyebrow">QUICK START</span><h2 id="manual-title">MoveMgr Manual</h2></div><button class="close" aria-label="Close" onclick={() => (dialog = null)}>×</button></header><div class="manual-body"><p>MoveMgr organizes files from source folders into target folders using reusable project rules.</p><ol><li>Add a project, then choose its source and target folders.</li><li>Set source rules to filter extensions, names, subfolders, and hidden files.</li><li>Set target rules to extract a key from each filename and choose the destination strategy.</li><li>Use Preview to review every planned move. Nothing changes until you start the move.</li><li>Export your settings if you want a portable backup.</li></ol><p class="manual-note">If MoveMgr makes your file chores a little less annoying, please give the project a GitHub star. It would make this tiny organizer very happy ★</p></div><footer class="modal-actions"><button onclick={() => (dialog = null)}>Close</button></footer></div></div>
{/if}
