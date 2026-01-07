# LLM Usage

## LLM helper (opt-in)

## Opt in
- LLM calls are off until you provide an OpenAI API key.
- Skip every prompt to keep running fully offline.

## API keys
- `repeater llm --set <KEY>` saves the key via the OS keyring (`com.repeat/openai:default`), so macOS Keychain/Windows Credential Manager/libsecret hold it securely.
- `REPEAT_OPENAI_API_KEY` overrides the keyring for temporary runs.
- `repeater llm --test` confirms the key with OpenAI, `repeater llm --clear` forgets it instantly.

## Cloze generation
- Run `repeater drill <deck>`; if any `C:` cards lack `[]`, you’ll be asked whether to send that text to OpenAI (`gpt-5-nano`).
- Choosing “yes” streams the card text, gets a single suggested deletion, and patches the file before the drill continues.
- Choosing “no” keeps the cards untouched and the feature idle.
