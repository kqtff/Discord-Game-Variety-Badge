# Discord Game Variety Badge

A small Windows app that makes Discord detect several verified games running at
once. Pick how many: **5, 10, 15, 20, 30, 40, 50, 80, 100 or 100+**, press
**Start**, and Discord sees that many games running.

It uses the same method as
[discord-quest-completer](https://github.com/markterence/discord-quest-completer):
Discord detects games by the name and folder of the running `.exe`, so the app
creates a tiny stand-in executable for each game and runs it. No real games are
downloaded.

## How to use

1. Download `discord-game-variety-badge.exe` from the Releases page (or the latest
   build under the Actions tab).
2. Put it in its own folder where you can write files (not `C:\Program Files`).
   It creates a `games\` folder next to itself.
3. Open Discord, then the app. Choose a number and press **Start**.
4. Press **Stop all** (or just close the app) to stop every game.

Options:

- **Popular games first.** Well-known games (Overwatch, Fortnite, Minecraft,
  VALORANT, …) are always picked first; the rest are filled in at random.
  **Shuffle** picks a different random set.
- **100+.** Choose any number above 100, up to every game in the list.
- **Stop automatically after N minutes.**
- **Log → Delete games folder** removes the stand-in files.

## Notes

- The game list comes from Discord (`/api/applications/detectable`) each time
  the app opens, with a saved copy and a built-in copy as fallbacks. Only games
  with a Windows executable can be used (about 10,500).
- Each stand-in is a hard link to the app's own `.exe`, so 100 games use no
  extra disk space. On drives without hard links (FAT32/exFAT) they are copies
  instead, about 7 MB each.
- All stand-ins close automatically when the app closes, even if it crashes.
- How many games Discord actually *displays* at once is up to Discord. Your
  profile may only show one or a few of them.

## Build from source

```bash
cargo build --release
# target/release/discord-game-variety-badge.exe
```

Cross-compiling from Linux: `rustup target add x86_64-pc-windows-gnu`, install
MinGW (`mingw-w64`), then `cargo build --release --target x86_64-pc-windows-gnu`.

`assets/gamelist.min.json` is the built-in fallback list (Windows executables
only) and `assets/popular.json` is the "popular first" list; every name in it
must match Discord's list exactly.

## Disclaimer

Faking game activity likely breaks Discord's Terms of Service and could get
your account flagged. Use at your own risk. Not affiliated with Discord Inc.

## Credits

Approach based on
[discord-quest-completer](https://github.com/markterence/discord-quest-completer)
by Mark Terence Tiglao (MIT License).
