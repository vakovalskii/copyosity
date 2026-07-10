# Local AI (Ollama) onboarding

Read when changing Settings local-AI UI, Ollama status checks, or model download flows.

Related code: `src/routes/settings/+page.svelte`, Ollama helpers in `src/lib/`.

## Product rules

1. If Ollama is not installed, do not silently install it.
2. Show a clear onboarding state with a download action and short instructions.
3. If Ollama is installed but not running, show that state separately and offer a start/check-again action.
4. If Ollama is installed but the selected model is missing, the app may offer to download the model directly.
5. If both Ollama and the model are ready, show a clear ready state.

## Expected user-facing states

- `Ollama not installed`
- `Ollama installed, server not running`
- `Model not installed`
- `Local AI ready`

## Expected actions

- `Download Ollama`
- `Start Ollama`
- `Download model`
- `Check again`
- `Change model`

## Policy

- System-level Ollama installation should be explicit and user-approved.
- Model downloads may be initiated from inside the app once Ollama is present.
- The UI should always explain what is missing: runtime, server, or model.
