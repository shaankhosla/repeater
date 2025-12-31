precommit:
    cargo sqlx prepare
    cargo fmt --all -- --check
    cargo clippy --fix --allow-dirty --allow-staged
    cargo machete
    cargo test

delete_db:
    -rm "/Users/shaankhosla/Library/Application Support/repeat/cards.db"
    -touch "/Users/shaankhosla/Library/Application Support/repeat/cards.db"
    DATABASE_URL="sqlite:///Users/shaankhosla/Library/Application Support/repeat/cards.db" sqlx migrate run

create:
    cargo run -- create /Users/shaankhosla/Desktop/sample_repeat_cards/test.md

check:
    cargo run -- check /Users/shaankhosla/Desktop/sample_repeat_cards/

drill:
    cargo run -- drill /Users/shaankhosla/Desktop/sample_repeat_cards/

import:
    cargo run -- import "/Users/shaankhosla/Downloads/All Decks.apkg" "/Users/shaankhosla/Desktop/anki_export/"

release:
    just precommit
    ./scripts/release.sh
