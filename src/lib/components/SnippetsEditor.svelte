<script lang="ts">
  import { onMount, tick } from "svelte";
  import ChevronDown from "$lib/components/ChevronDown.svelte";
  import type { Snippet, SnippetFolder } from "$lib/types";
  import { confirmDestructive } from "$lib/confirm";
  import {
    createSnippet,
    createSnippetFolder,
    deleteSnippet,
    deleteSnippetFolder,
    getSnippetFolders,
    getSnippets,
    renameSnippetFolder,
    updateSnippet,
  } from "$lib/api";
  import {
    isSnippetFolderExpanded,
    loadCollapsedSnippetFolderIds,
    pruneCollapsedSnippetFolderIds,
    saveCollapsedSnippetFolderIds,
  } from "$lib/snippet-folders-ui";

  let folders = $state<SnippetFolder[]>([]);
  let snippets = $state<Snippet[]>([]);
  let loading = $state(true);
  let error = $state("");

  let newFolderName = $state("");
  let drafts = $state<Record<number, { title: string; content: string }>>({});
  let editing = $state<Record<number, { title: string; content: string }>>({});
  let collapsedFolderIds = $state<Set<number>>(new Set());
  let renamingFolderId = $state<number | null>(null);
  let renameDraft = $state("");
  let renameInputEl = $state<HTMLInputElement | null>(null);

  function snippetsIn(folderId: number): Snippet[] {
    return snippets.filter((s) => s.folder_id === folderId);
  }

  function folderExpanded(folderId: number): boolean {
    return isSnippetFolderExpanded(folderId, collapsedFolderIds);
  }

  function persistCollapsedFolders() {
    saveCollapsedSnippetFolderIds(collapsedFolderIds);
  }

  function toggleFolder(folder: SnippetFolder) {
    if (renamingFolderId === folder.id) {
      void commitRenameFolder(folder);
    }
    const next = new Set(collapsedFolderIds);
    if (next.has(folder.id)) {
      next.delete(folder.id);
    } else {
      next.add(folder.id);
    }
    collapsedFolderIds = next;
    persistCollapsedFolders();
  }

  function expandFolder(folderId: number) {
    if (!collapsedFolderIds.has(folderId)) return;
    const next = new Set(collapsedFolderIds);
    next.delete(folderId);
    collapsedFolderIds = next;
    persistCollapsedFolders();
  }

  function pruneCollapsedFolders(activeFolderIds: number[]) {
    const next = pruneCollapsedSnippetFolderIds(collapsedFolderIds, activeFolderIds);
    if (next.size === collapsedFolderIds.size) return;
    collapsedFolderIds = next;
    persistCollapsedFolders();
  }

  async function reload() {
    loading = true;
    error = "";
    try {
      [folders, snippets] = await Promise.all([getSnippetFolders(), getSnippets()]);
      for (const folder of folders) {
        drafts[folder.id] ??= { title: "", content: "" };
      }
      pruneCollapsedFolders(folders.map((folder) => folder.id));
    } catch (e) {
      error = `Failed to load snippets: ${e}`;
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    collapsedFolderIds = loadCollapsedSnippetFolderIds();
    void reload();
  });

  async function addFolder() {
    const name = newFolderName.trim();
    if (!name) return;
    try {
      const folderId = await createSnippetFolder(name);
      newFolderName = "";
      await reload();
      expandFolder(folderId);
    } catch (e) {
      error = `${e}`;
    }
  }

  function startRenameFolder(folder: SnippetFolder) {
    renamingFolderId = folder.id;
    renameDraft = folder.name;
    void tick().then(() => renameInputEl?.focus());
  }

  function cancelRenameFolder() {
    renamingFolderId = null;
    renameDraft = "";
  }

  async function commitRenameFolder(folder: SnippetFolder) {
    const trimmed = renameDraft.trim();
    cancelRenameFolder();
    await renameFolder(folder, trimmed);
  }

  function handleRenameKeydown(e: KeyboardEvent, folder: SnippetFolder) {
    if (e.key === "Enter") {
      e.preventDefault();
      void commitRenameFolder(folder);
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelRenameFolder();
    }
  }

  async function renameFolder(folder: SnippetFolder, name: string) {
    const trimmed = name.trim();
    if (!trimmed || trimmed === folder.name) return;
    try {
      await renameSnippetFolder(folder.id, trimmed);
      await reload();
    } catch (e) {
      error = `${e}`;
    }
  }

  async function removeFolder(folder: SnippetFolder) {
    const confirmed = await confirmDestructive({
      title: `Delete "${folder.name}"?`,
      message: "This deletes the folder and all snippets inside it.",
      confirmLabel: "Delete",
      destructiveConfirm: true,
    });
    if (!confirmed) return;
    try {
      await deleteSnippetFolder(folder.id);
      await reload();
    } catch (e) {
      error = `${e}`;
    }
  }

  async function addSnippet(folderId: number) {
    const draft = drafts[folderId];
    if (!draft) return;
    const title = draft.title.trim();
    if (!title) return;
    try {
      await createSnippet(folderId, title, draft.content);
      drafts[folderId] = { title: "", content: "" };
      await reload();
      expandFolder(folderId);
    } catch (e) {
      error = `${e}`;
    }
  }

  function startEdit(snippet: Snippet) {
    expandFolder(snippet.folder_id);
    editing[snippet.id] = { title: snippet.title, content: snippet.content };
  }

  function cancelEdit(id: number) {
    delete editing[id];
    editing = { ...editing };
  }

  async function saveEdit(id: number) {
    const draft = editing[id];
    if (!draft) return;
    const title = draft.title.trim();
    if (!title) return;
    try {
      await updateSnippet(id, title, draft.content);
      cancelEdit(id);
      await reload();
    } catch (e) {
      error = `${e}`;
    }
  }

  async function removeSnippet(snippet: Snippet) {
    try {
      await deleteSnippet(snippet.id);
      await reload();
    } catch (e) {
      error = `${e}`;
    }
  }
</script>

<div class="snippets-stack">
  {#if error}
    <div class="status-hint fail" role="alert">{error}</div>
  {/if}

  <div class="inset-list">
    <div class="form-field">
      <div class="form-inline">
        <input
          class="form-input"
          type="text"
          placeholder="New folder name (e.g. Addresses, Prompts)"
          aria-label="New snippet folder name"
          bind:value={newFolderName}
          onkeydown={(e) => e.key === "Enter" && addFolder()}
        />
        <button
          class="form-btn form-btn-secondary app-btn"
          type="button"
          onclick={addFolder}
          disabled={!newFolderName.trim()}
        >
          Add folder
        </button>
      </div>
    </div>
  </div>

  {#if loading}
    <p class="form-hint">Loading…</p>
  {:else if folders.length === 0}
    <p class="form-hint">No folders yet. Create one above to start adding snippets.</p>
  {:else}
    <div class="inset-list snip-folders-list">
      {#each folders as folder (folder.id)}
        {@const expanded = folderExpanded(folder.id)}
        {@const folderSnippets = snippetsIn(folder.id)}
        {@const renaming = renamingFolderId === folder.id}
        <div
          class="snip-folder-header"
          class:snip-folder-header--renaming={renaming}
          aria-labelledby={renaming ? undefined : `snip-folder-label-${folder.id}`}
        >
          <button
            class="snip-folder-chevron-toggle app-btn"
            type="button"
            aria-expanded={expanded}
            aria-controls={`snip-folder-panel-${folder.id}`}
            aria-label={expanded ? `Collapse ${folder.name}` : `Expand ${folder.name}`}
            onclick={() => toggleFolder(folder)}
          >
            <span class="snip-folder-chevron" class:collapsed={!expanded}>
              <ChevronDown />
            </span>
          </button>
          {#if renaming}
            <input
              class="snip-folder-rename-input"
              type="text"
              aria-label="Folder name"
              bind:value={renameDraft}
              bind:this={renameInputEl}
              onkeydown={(e) => handleRenameKeydown(e, folder)}
              onblur={() => commitRenameFolder(folder)}
            />
          {:else}
            <button
              class="snip-folder-title-toggle app-btn"
              type="button"
              id={`snip-folder-label-${folder.id}`}
              aria-expanded={expanded}
              aria-controls={`snip-folder-panel-${folder.id}`}
              onclick={() => toggleFolder(folder)}
              ondblclick={(e) => {
                e.preventDefault();
                startRenameFolder(folder);
              }}
            >
              {folder.name}
            </button>
          {/if}
          <span class="snip-folder-count" aria-label={`${folderSnippets.length} snippets`}>
            {folderSnippets.length}
          </span>
          {#if renaming}
            <button
              class="action-btn app-btn snip-folder-rename-btn"
              type="button"
              aria-label="Cancel rename"
              title="Cancel"
              onmousedown={(e) => {
                e.preventDefault();
                cancelRenameFolder();
              }}
            >
              <svg
                class="action-icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          {:else}
            <button
              class="action-btn app-btn snip-folder-rename-btn"
              type="button"
              aria-label={`Rename folder ${folder.name}`}
              title="Rename"
              onclick={(e) => {
                e.stopPropagation();
                startRenameFolder(folder);
              }}
            >
              <svg
                class="action-icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <path d="M12 20h9" />
                <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
              </svg>
            </button>
          {/if}
        </div>

        <div
          id={`snip-folder-panel-${folder.id}`}
          class="snip-folder-panel"
          hidden={!expanded}
        >
        {#if expanded}
          {#if folderSnippets.length === 0}
            <p class="snip-folder-empty form-hint">
              No snippets in this folder yet.
            </p>
          {/if}

          {#each folderSnippets as snippet (snippet.id)}
            {#if editing[snippet.id]}
              <div class="form-field">
                    <span class="form-label">Edit snippet</span>
                    <input class="form-input" type="text" bind:value={editing[snippet.id].title} />
                    <textarea
                      class="form-textarea"
                      rows="3"
                      bind:value={editing[snippet.id].content}
                    ></textarea>
                    <div class="form-inline snip-row-actions">
                      <button
                        class="form-btn form-btn-secondary app-btn"
                        type="button"
                        onclick={() => saveEdit(snippet.id)}
                      >
                        Save
                      </button>
                      <button
                        class="form-btn form-btn-ghost app-btn"
                        type="button"
                        onclick={() => cancelEdit(snippet.id)}
                      >
                        Cancel
                      </button>
                    </div>
              </div>
            {:else}
              <div class="snip-entry-row">
                    <div class="snip-entry-copy">
                      <div class="snip-entry-title">{snippet.title}</div>
                      <div class="snip-entry-preview">{snippet.content}</div>
                    </div>
                    <div class="snip-row-actions">
                      <button
                        class="action-btn app-btn"
                        type="button"
                        onclick={() => startEdit(snippet)}
                        aria-label={`Edit snippet ${snippet.title}`}
                        title="Edit"
                      >
                        <svg
                          class="action-icon"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          stroke-width="2"
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          aria-hidden="true"
                        >
                          <path d="M12 20h9" />
                          <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
                        </svg>
                      </button>
                      <button
                        class="action-btn app-btn delete"
                        type="button"
                        onclick={() => removeSnippet(snippet)}
                        aria-label={`Delete snippet ${snippet.title}`}
                        title="Delete"
                      >
                        <svg
                          class="action-icon"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          stroke-width="2"
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          aria-hidden="true"
                        >
                          <polyline points="3 6 5 6 21 6" />
                          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                          <line x1="10" y1="11" x2="10" y2="17" />
                          <line x1="14" y1="11" x2="14" y2="17" />
                        </svg>
                      </button>
                    </div>
              </div>
            {/if}
          {/each}

          {#if drafts[folder.id]}
            <div class="form-field">
                  <input
                    class="form-input"
                    type="text"
                    placeholder="Snippet title (e.g. Work email)"
                    aria-label="Snippet title"
                    bind:value={drafts[folder.id].title}
                  />
                  <textarea
                    class="form-textarea"
                    rows="2"
                    placeholder="Content to paste…"
                    aria-label="Snippet content"
                    bind:value={drafts[folder.id].content}
                  ></textarea>
                  <button
                    class="form-btn form-btn-secondary app-btn snip-add-btn"
                    type="button"
                    onclick={() => addSnippet(folder.id)}
                    disabled={!drafts[folder.id].title.trim()}
                  >
                    Add
                  </button>
            </div>
          {/if}

          <div class="snip-folder-footer">
            <button
              class="snip-folder-delete app-btn"
              type="button"
              onclick={() => removeFolder(folder)}
              aria-label={`Delete folder ${folder.name} and all snippets inside`}
            >
              Delete folder…
            </button>
          </div>
        {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .snippets-stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-stack);
    min-width: 0;
  }

  .snip-folders-list {
    min-width: 0;
  }

  .snip-folder-panel[hidden] {
    display: none;
  }

  .snippets-stack :global(.inset-list > .snip-folder-header) {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: auto 1fr auto auto;
    align-items: center;
    gap: var(--space-stack);
    min-height: 2rem;
    padding: var(--space-control-y) var(--inset-list-pad-inline);
  }

  .snip-folder-chevron-toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--icon-size-chevron);
    height: var(--icon-size-chevron);
    border: none;
    background: transparent;
    cursor: pointer;
    padding: 0;
    color: inherit;
    -webkit-tap-highlight-color: transparent;
  }

  .snip-folder-title-toggle,
  .snip-folder-rename-input {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
    height: 1.5rem;
    margin: 0;
    padding: 0 var(--space-field);
    border: 1px solid transparent;
    border-radius: var(--radius-control-sm);
    font: inherit;
    font-size: var(--font-size-md);
    font-weight: 600;
    line-height: 1.25;
    color: var(--color-text-primary);
  }

  .snip-folder-title-toggle {
    background: transparent;
    cursor: pointer;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    -webkit-tap-highlight-color: transparent;
  }

  .snip-folder-chevron {
    display: inline-flex;
    flex-shrink: 0;
    transition: transform var(--duration-fast) var(--ease-interactive);
  }

  .snip-folder-chevron.collapsed {
    transform: rotate(-90deg);
  }

  .snip-folder-rename-input {
    background: transparent;
  }

  .snip-folder-rename-input:focus {
    outline: none;
    border-color: var(--border-control-focus);
    background: var(--surface-control-focus);
    box-shadow: var(--ring-control-focus);
  }

  .snip-folder-count {
    flex-shrink: 0;
    min-width: 1.25rem;
    padding: 0 var(--space-field);
    border-radius: var(--radius-pill);
    background: var(--surface-6);
    font-size: var(--font-size-xs);
    font-weight: 600;
    line-height: 1.5;
    color: var(--color-text-subtle);
    text-align: center;
  }

  .snip-folder-rename-btn {
    flex-shrink: 0;
  }

  .snip-folder-empty {
    margin: 0;
    padding: var(--space-control-y) var(--inset-list-pad-inline);
  }

  .snippets-stack :global(.inset-list > .snip-entry-row) {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-stack);
    min-height: 2rem;
    padding: var(--space-control-y) var(--inset-list-pad-inline);
  }

  .snippets-stack :global(.inset-list > .snip-folder-footer) {
    box-sizing: border-box;
    padding: var(--space-control-y) var(--inset-list-pad-inline);
  }

  .snip-folder-delete {
    border: none;
    background: transparent;
    cursor: pointer;
    font: inherit;
    font-size: var(--font-size-xs);
    line-height: 1.25;
    color: var(--color-text-subtle);
    padding: 0;
    -webkit-tap-highlight-color: transparent;
    transition: color var(--duration-fast) var(--ease-interactive);
  }

  .snip-folder-delete:hover:not(:disabled) {
    color: var(--color-danger-text);
  }

  .snip-entry-copy {
    min-width: 0;
    flex: 1 1 auto;
  }

  .snip-entry-title {
    font-size: var(--font-size-md);
    font-weight: 600;
    line-height: 1.25;
    color: var(--color-text-primary);
  }

  .snip-entry-preview {
    margin-top: var(--space-field);
    font-size: var(--font-size-sm);
    line-height: var(--line-height-hint);
    color: var(--color-text-subtle);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .snip-row-actions {
    display: inline-flex;
    align-items: center;
    gap: var(--space-chip-gap);
    flex-shrink: 0;
  }

  .snip-add-btn {
    align-self: flex-start;
  }
</style>
