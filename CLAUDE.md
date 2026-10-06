# Stepwise

Offline Rust CLI and desktop exercises for Python evaluation, propositional logic and natural deduction.

- Use tabs and explicit types. Keep changes focused; do not add AI co-author lines.
- Public README and `docs/` teach usage and development. Internal status, evidence ledgers, design rationale and plans belong in the private `notes/` submodule, never in public documentation or commit messages. Internal evidence lives at `notes/STATUS.md`; no root status file. Do not copy private note contents or tracker links into public files.
- For internal work, check the project's execution tracker and private notes first. Update only `notes`' configured project branch: inspect its worktree, then `git submodule update --init --remote --merge -- notes`. Preserve local changes and stop on divergence. Follow the notes repository's own protected submission workflow, push its commit successfully, then update the parent gitlink. Public builds and tests must work without initializing notes.
- One real instruction file: this one. `AGENTS.md` and `.github/copilot-instructions.md` are symlinks to it.

## Build and test

- Native gates: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`. The default workspace member is the CLI/library, not the Tauri shell.
- Python semantics changes also run `cargo test --locked --test python_oracle -- --ignored --nocapture`; terminal adapter changes also run `cargo test --locked --test terminal -- --ignored --nocapture`. They require CPython and, for terminal tests, a pty; neither is a runtime dependency.
- Desktop gates and prerequisites are in `docs/development.md`. Rust tests generate `src/desktop/ui/src/lib/protocol/`; regenerate declarations instead of editing them.
- CI: `lint.yml` (format and static checks, Linux only) and `test.yml` run on pull requests, only for the parts a pull request touches (`.github/scripts/changes.py` maps paths to checks; keep it in step when files move). Packaging is `package-cli.yml` and `package-desktop.yml`, started by hand and independent; each runs its family's tests first. Keep checks over extracted archives and generated license notices intact. Packaging also signs and notarizes the macOS builds with Developer ID and fails without the Apple secrets; publication is a separate action. See `docs/distribution.md`.

## Code boundaries

- `src/python` and `src/logic` own syntax, operators, precedence, explanations and generation grammars. In `src/core`, only `language.rs` names those modules.
- `src/core` owns teaching steps, validation and replay. `allowed_steps` is the student's choice set; `next_step` is a hint/trace reference. Never use the reference to preselect an answer. Bump `RULES` when the allowed set changes.
- `src/app` owns lessons, selection, drafts, history and progress snapshots; no terminal/window dependencies or file I/O. Device-specific wording belongs in each adapter's exhaustive match over `Notice`.
- `src/tui` renders inline in the primary terminal buffer. No fullscreen, tree pane or Tab views. Keep the previous expression above the answer blank and append accepted history.
- `src/desktop/*.rs` adapts app state through `Command` and `View`; no Tauri or file I/O. `src/desktop/src-tauri` owns window/files/preferences; `src/desktop/ui` renders and forwards input, without duplicating teaching rules. Keep draft editions and IME handling intact.
- Question sets are versioned TOML (`src/exercises.rs`); proofs contain premises and a goal, never a worked solution. Complete proofs belong only in tests. Keep the TOML example and rule table in `docs/question-sets.md` and `docs/reference.md` covered by their existing tests.
- Preserve the documented interaction and semantics in `docs/usage.md` and `docs/reference.md`, including one-click grouping, simultaneous variable substitution, optional short circuit per step and final signed-value completion. Shared rules must behave the same in both front ends.

## Project notes submission

Project notes live directly at the configured branch root; keep vault automation and configuration on the notes remote's master branch. Follow that repository's `项目接入.md` for initialization, updates and conflict handling. Preserve existing local edits.

Install or refresh the vault-owned submission tools in Git metadata, including in new clones:

```fish
git -C notes fetch origin refs/heads/master:refs/remotes/origin/master
set notes_common_gitdir (git -C notes rev-parse --path-format=absolute --git-common-dir)
git -C notes show origin/master:.github/scripts/install_push_hook.py > "$notes_common_gitdir/install_push_hook.py"
python3 "$notes_common_gitdir/install_push_hook.py" --repo notes --source-ref origin/master
```

After committing specific note files, use `python3 "$notes_common_gitdir/hooks/notes-boundary/submit_project.py" --repo notes`. It obtains the required remote boundary check before pushing the same commit. Only then commit and push this repository's `notes` gitlink.
