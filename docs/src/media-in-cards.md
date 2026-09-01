# Media in Cards

`repeater` scans every rendered card for media references (images, audio, and video). Images are drawn **directly in the terminal** when your terminal can display them; anything else — audio, video, or a terminal without graphics support — is one `O` keypress away from opening in your operating system’s default viewer/player.

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

## Inline images

In a terminal that supports a graphics protocol, an image on the side of the card you are
looking at is drawn in the card panel: the question or answer text sits on top, the picture
fills the space below it. Press `I` to give the picture the whole panel, and `I` again to go
back. Because the image is tied to the visible side of the card, a picture in the answer only
appears once you reveal it.

Supported protocols and the terminals that use them:

| Protocol | Terminals |
| --- | --- |
| Kitty graphics | Kitty (0.28+), Ghostty |
| iTerm2 inline images | iTerm2, WezTerm, Rio |
| Sixel | foot, xterm (`-ti 340`), mlterm |

Terminals with no graphics protocol — Alacritty, Konsole, macOS Terminal.app, Windows
conhost, Warp — are detected automatically and left alone: no image is drawn, and the footer
hint plus `O` behave exactly as they always have. Nothing is silently degraded to a blurry
approximation.

Control this with `--inline-images`:

- `auto` (default): draw images only where a real graphics protocol is available.
- `off`: never draw inline; always rely on `O`.
- `always`: also draw in terminals without a protocol, using coarse Unicode half-blocks.
  Usable for photographs, poor for diagrams or anything with text in it.

```sh
repeater drill flashcards/anatomy/ --inline-images always
```

Notes and limits:

- The format is detected from the file contents, not the extension, so a mislabeled file
  still displays.
- Animated GIFs show their first frame.
- Very large images are downscaled before display, and files over 32 MB are skipped.
- If an image cannot be displayed, the footer says why (`image not shown: …`) and the card
  still drills normally — you can press `O` to open it externally.

## Opening media during a drill

While drilling:

- The footer shows “media file found” whenever the current card links to supported media.
- Press `O` (uppercase or lowercase) to open it — on the question side or the answer side.
  When an image is being drawn inline, `O` opens that image; otherwise it opens the first
  attachment listed on the card.
- The file launches via the OS default handler (`open` on macOS, `xdg-open` on Linux, `start` on Windows), so whatever app normally opens that file type will appear.

If a file cannot be found you’ll see `File does not exist: …` in the terminal. Double-check the relative path from the deck file and ensure the media is synced locally.

Multiple attachments can be detected;
