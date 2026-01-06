precommit:
    SQLX_OFFLINE=true cargo fmt --all -- --check
    SQLX_OFFLINE=true cargo clippy --fix --allow-dirty --allow-staged
    SQLX_OFFLINE=true cargo machete
    SQLX_OFFLINE=true cargo test

delete_db:
    -rm "/Users/shaankhosla/Library/Application Support/repeater/cards.db"
    -touch "/Users/shaankhosla/Library/Application Support/repeater/cards.db"
    DATABASE_URL="sqlite:///Users/shaankhosla/Library/Application Support/repeater/cards.db" sqlx migrate run

create:
    cargo run -- create /Users/shaankhosla/Desktop/sample_repeater_cards/test.md

check:
    cargo run -- check /Users/shaankhosla/Desktop/sample_repeater_cards/

drill:
    cargo run -- drill /Users/shaankhosla/Desktop/sample_repeater_cards/

import:
    cargo run -- import "/Users/shaankhosla/Downloads/All Decks.apkg" "/Users/shaankhosla/Desktop/anki_export/"

release:
    just precommit
    ./scripts/release.sh
