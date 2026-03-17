# Repository Workflow

Project workflow rules:

1. After a release is finished, start the next iteration in a new branch.
2. New features, fixes, and experiments must be pushed to that branch.
3. After every code generation pass or manual code edit, test compilation of the app.
4. Keep git history and workflow notes up to date by committing changes clearly and updating this file together with `AGENTS.md`.

## Mandatory Checks

Use these commands after code changes:

```bash
npm run check
cd src-tauri && cargo check
```

## Expected Practice

- Do not continue feature development directly on the last release branch.
- Do not skip compilation checks after generating code.
- Do not leave workflow changes undocumented.

## Local AI Onboarding

When working on Ollama onboarding in the app, follow this product rule set:

1. If Ollama is not installed, do not silently install it.
2. Show a clear onboarding state with a download action and short instructions.
3. If Ollama is installed but not running, show that state separately and offer a start/check-again action.
4. If Ollama is installed but the selected model is missing, the app may offer to download the model directly.
5. If both Ollama and the model are ready, show a clear ready state.

Expected user-facing states:

- `Ollama not installed`
- `Ollama installed, server not running`
- `Model not installed`
- `Local AI ready`

Expected actions:

- `Download Ollama`
- `Start Ollama`
- `Download model`
- `Check again`
- `Change model`

Product policy:

- System-level Ollama installation should be explicit and user-approved.
- Model downloads may be initiated from inside the app once Ollama is present.
- The UI should always explain what is missing: runtime, server, or model.
