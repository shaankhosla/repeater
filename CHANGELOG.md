## [0.1.0] - 2026-01-18

### 🚀 Features

- [**breaking**] Make hashing more robust  (#51)
- Track files traversed in check command (#62)
- Rephrase questions with AI (#64)
- Latex math gets converted to unicode (#66)
- Improve cloze prompt user experience with better formatting and confirmation (#70)

### 🐛 Bug Fixes

- Add linux persistence for llm keys
- FSRS stats will show despite no cards being due (#69)
- Incorrect API key not failing (#71)
- `create` command works with new file (#72)

### 💼 Other

- *(deps)* Bump blake3 from 1.8.2 to 1.8.3 (#55)
- *(deps)* Bump serde_json from 1.0.148 to 1.0.149 (#57)
- *(deps)* Bump async-openai from 0.30.1 to 0.32.3 (#56)
- *(deps)* Bump keyring from 2.3.3 to 3.6.3 (#54)

### 🚜 Refactor

- Reorganize code (#58)
- Unify theming (#60)
- Improve cicd (#61)
- Reduce user prompts for keychain password (#63)
- Improve Anki import significantly with logging and code refactor (#65)
- Break out user prompts to separate file (#67)
- Cicd and tests (#68)

### 📚 Documentation

- Readme improvements (#52)
- Show star history

### 🧪 Testing

- Increase test coverage (#59)

### ⚙️ Miscellaneous Tasks

- Update author/license
- Fix failing cicd
- Add install
