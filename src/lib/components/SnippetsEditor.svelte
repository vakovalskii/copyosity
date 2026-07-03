<script lang="ts">
  import { onMount } from "svelte";
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

  let folders = $state<SnippetFolder[]>([]);
  let snippets = $state<Snippet[]>([]);
  let loading = $state(true);
  let error = $state("");

  // New-folder input
  let newFolderName = $state("");

  // Per-folder "add snippet" draft, keyed by folder id
  let drafts = $state<Record<number, { title: string; content: string }>>({});

  // Snippet being edited (id -> working copy)
  let editing = $state<Record<number, { title: string; content: string }>>({});

  function snippetsIn(folderId: number): Snippet[] {
    return snippets.filter((s) => s.folder_id === folderId);
  }

  async function reload() {
    loading = true;
    error = "";
    try {
      [folders, snippets] = await Promise.all([getSnippetFolders(), getSnippets()]);
      // Ensure every folder has an add-snippet draft so inputs can bind directly.
      for (const folder of folders) {
        drafts[folder.id] ??= { title: "", content: "" };
      }
    } catch (e) {
      error = `Failed to load snippets: ${e}`;
    } finally {
      loading = false;
    }
  }

  onMount(reload);

  async function addFolder() {
    const name = newFolderName.trim();
    if (!name) return;
    try {
      await createSnippetFolder(name);
      newFolderName = "";
      await reload();
    } catch (e) {
      error = `${e}`;
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
    } catch (e) {
      error = `${e}`;
    }
  }

  function startEdit(snippet: Snippet) {
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
    const confirmed = await confirmDestructive({
      title: `Delete "${snippet.title}"?`,
      confirmLabel: "Delete",
      destructiveConfirm: true,
    });
    if (!confirmed) return;
    try {
      await deleteSnippet(snippet.id);
      await reload();
    } catch (e) {
      error = `${e}`;
    }
  }
</script>

<div class="snippets-editor">
  {#if error}
    <p class="snip-error" role="alert">{error}</p>
  {/if}

  <div class="snip-add-folder">
    <input
      class="form-input"
      type="text"
      placeholder="New folder name (e.g. Addresses, Prompts)"
      bind:value={newFolderName}
      onkeydown={(e) => e.key === "Enter" && addFolder()}
    />
    <button class="app-btn" type="button" onclick={addFolder} disabled={!newFolderName.trim()}>
      Add folder
    </button>
  </div>

  {#if loading}
    <p class="snip-muted">Loading…</p>
  {:else if folders.length === 0}
    <p class="snip-muted">
      No snippet folders yet. Create one above, then add snippets — they'll appear in the quick
      menu for two-click paste.
    </p>
  {:else}
    {#each folders as folder (folder.id)}
      <section class="snip-folder">
        <header class="snip-folder-head">
          <input
            class="form-input snip-folder-name"
            type="text"
            value={folder.name}
            onblur={(e) => renameFolder(folder, (e.currentTarget as HTMLInputElement).value)}
          />
          <button
            class="app-btn snip-danger"
            type="button"
            onclick={() => removeFolder(folder)}
            aria-label={`Delete folder ${folder.name}`}
          >
            Delete
          </button>
        </header>

        <ul class="snip-list">
          {#each snippetsIn(folder.id) as snippet (snippet.id)}
            <li class="snip-item">
              {#if editing[snippet.id]}
                <div class="snip-edit">
                  <input class="form-input" type="text" bind:value={editing[snippet.id].title} />
                  <textarea class="form-input snip-textarea" rows="3" bind:value={editing[snippet.id].content}></textarea>
                  <div class="snip-edit-actions">
                    <button class="app-btn" type="button" onclick={() => saveEdit(snippet.id)}>Save</button>
                    <button class="app-btn snip-ghost" type="button" onclick={() => cancelEdit(snippet.id)}>Cancel</button>
                  </div>
                </div>
              {:else}
                <div class="snip-row">
                  <div class="snip-text">
                    <div class="snip-title">{snippet.title}</div>
                    <div class="snip-preview">{snippet.content}</div>
                  </div>
                  <div class="snip-row-actions">
                    <button class="app-btn snip-ghost" type="button" onclick={() => startEdit(snippet)}>Edit</button>
                    <button
                      class="app-btn snip-danger"
                      type="button"
                      onclick={() => removeSnippet(snippet)}
                      aria-label={`Delete snippet ${snippet.title}`}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              {/if}
            </li>
          {/each}
        </ul>

        {#if drafts[folder.id]}
          <div class="snip-add">
            <input
              class="form-input"
              type="text"
              placeholder="Snippet title (e.g. Work email)"
              bind:value={drafts[folder.id].title}
            />
            <textarea
              class="form-input snip-textarea"
              rows="2"
              placeholder="Content to paste…"
              bind:value={drafts[folder.id].content}
            ></textarea>
            <button
              class="app-btn"
              type="button"
              onclick={() => addSnippet(folder.id)}
              disabled={!drafts[folder.id].title.trim()}
            >
              Add snippet
            </button>
          </div>
        {/if}
      </section>
    {/each}
  {/if}
</div>

<style>
  .snippets-editor {
    display: flex;
    flex-direction: column;
    gap: var(--space-stack, 12px);
  }
  .snip-error {
    color: var(--color-danger-text, #d33);
    font-size: var(--font-size-sm);
    margin: 0;
  }
  .snip-muted {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    margin: 0;
  }
  .snip-add-folder {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .snip-add-folder .form-input {
    flex: 1 1 auto;
  }
  .snip-folder {
    border: 1px solid var(--surface-border, rgb(128 128 128 / 25%));
    border-radius: var(--radius-control, 10px);
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--surface-3, transparent);
  }
  .snip-folder-head {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .snip-folder-name {
    flex: 1 1 auto;
    font-weight: 600;
  }
  .snip-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .snip-item {
    border-radius: var(--radius-control-sm, 8px);
    background: var(--surface-5, rgb(128 128 128 / 8%));
    padding: 8px 10px;
  }
  .snip-row,
  .snip-row-actions,
  .snip-edit-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .snip-row {
    justify-content: space-between;
  }
  .snip-text {
    min-width: 0;
  }
  .snip-title {
    font-weight: 600;
    font-size: var(--font-size-sm);
  }
  .snip-preview {
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 340px;
  }
  .snip-edit,
  .snip-add {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .snip-add {
    border-top: 1px dashed var(--surface-border, rgb(128 128 128 / 25%));
    padding-top: 10px;
  }
  .snip-textarea {
    resize: vertical;
    font-family: inherit;
  }
  .snip-ghost {
    opacity: 0.85;
  }
  .snip-danger {
    color: var(--color-danger-text, #d33);
  }
</style>
