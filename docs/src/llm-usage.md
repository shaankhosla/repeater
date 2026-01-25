# LLM Usage

## LLM helper (opt-in)

## Opt in
- LLM calls are off until you configure a provider.
- Skip every prompt to keep running fully offline.

## Providers and config
- `repeater llm --set` walks you through provider, base URL (when needed), API key, and model selection.
- Config is stored in a local auth file under your OS data directory (for example, `~/Library/Application Support/repeater/auth.json` on macOS).
- `OPENAI_API_KEY` overrides the stored key for temporary runs.
- `repeater llm --test` confirms the config by listing models, `repeater llm --clear` forgets it instantly.

## Ollama (local LLM) compatibility
- Ollama works because `repeater` speaks OpenAI-compatible APIs.
- Use `repeater llm --set`, choose "Other", then set the base URL to `http://localhost:11434/v1/`.
- Leave the API key blank unless your proxy requires one.
- Make sure the model is available in Ollama (for example, `ollama pull llama3.1`), then select it from the model list.

## Cloze generation
- Run `repeater drill <deck>`; if any `C:` cards lack `[]`, `repeater` sends that text to your configured provider and patches the file before the drill continues.
- Leave the API key blank (or skip configuring a provider) to keep the feature idle.

Example cloze auto-fix:

```md
C: The capital of Japan is Tokyo.
```

When the drill starts, the LLM will turn it into something like:

```md
C: The capital of Japan is [Tokyo].
```

## Question rephrasing
- Run `repeater drill <deck> --rephrase` to rephrase basic `Q:` questions before the session starts.
- The original answers are provided as context but are not revealed in the rewritten questions.

Example rephrase:

```md
Q: What is the powerhouse of the cell?
A: The mitochondrion.
```

The LLM will rewrite the question only, for example:

```md
Q: Which organelle is known as the cell's powerhouse?
A: The mitochondrion.
```
