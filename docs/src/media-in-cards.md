# Media in Cards

`repeater` scans every rendered card for media references (images, audio, and video). When the drill UI detects at least one supported file, you can press `O` before revealing the answer to open the first attachment in your operating system’s default viewer/player. This keeps cards lightweight in the terminal while still letting you jump into richer references on demand.

## Supported formats

The following file extensions are detected:

- **Images:** `jpg`, `jpeg`, `png`, `gif`, `webp`, `bmp`
- **Audio:** `mp3`, `wav`, `ogg`, `flac`, `m4a`
- **Video:** `mp4`, `webm`, `mkv`, `mov`, `avi`

Other links remain untouched so regular hyperlinks still work in your Markdown outside the drill UI.

## Referencing media in Markdown

Use normal Markdown syntax for images (`![Alt](path/to/file.png)`) or links (`[label](path/to/file.mp3)`). `repeater` reads the destination path and decides if it looks like media based on the extension.

Relative paths are resolved from the directory that contains the deck file. For example, if your deck lives at `notes/physics/waves.md`, then:

```markdown
![Standing Wave](figures/wave.png)
[audio](../audio/tone.mp3)
```

will look for `notes/physics/figures/wave.png` and `notes/audio/tone.mp3`. Absolute paths work too, but keeping media alongside your decks makes them easier to sync and move.

## Opening media during a drill

While drilling:

- The footer shows “media file found” whenever the current card links to supported media.
- Press `O` (uppercase or lowercase) before revealing the answer to open the first listed file.
- The file launches via the OS default handler (`open` on macOS, `xdg-open` on Linux, `start` on Windows), so whatever app normally opens that file type will appear.

If a file cannot be found you’ll see `File does not exist: …` in the terminal. Double-check the relative path from the deck file and ensure the media is synced locally.

Multiple attachments can be detected, and the first one will open today; broader selection support is on the roadmap.
