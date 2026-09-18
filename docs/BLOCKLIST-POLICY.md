# Blocklist policy

The list is the product. These are its rules; a PR that breaks one is wrong
even if it blocks more.

1. **Exact hostnames only.** `generativelanguage.googleapis.com`, never
   `googleapis.com`. `api2.cursor.sh`, never `cursor.sh` (that would break
   the editor's own updates and docs). Bare second-level domains are allowed
   only when the whole site is the chatbot (`claude.ai`, `chatgpt.com`).
2. **No wildcards, no paths.** A hosts file cannot express either, and a
   rule we cannot enforce is a lie in the list.
3. **Kill only single-purpose processes.** Ollama, `claude`, `codex`,
   ChatGPT.app: nothing to lose. Cursor, VS Code, Zed, Xcode: never. Their
   AI dies behind the network block; their unsaved buffers do not.
4. **Exact basenames for processes.** `claude`, not anything containing
   "claude". App bundles match as a path component (`ChatGPT.app`), which
   also catches their helpers.
5. **Categories mean what a normal person thinks they mean.** "Chat apps
   and websites", "AI in my code editor and terminal", "AI running on my
   Mac". If an entry needs a footnote, it is in the wrong category.
6. **Adding is cheap, removing is a bug report.** When a new chatbot
   launches, add it. When a block breaks something unrelated, that is a
   release-blocking bug.
7. **Version bumps.** Every change increments `version`; the daemon only
   replaces its list with a higher version, so a bad publish cannot roll a
   good list back.

The daemon validates every list it loads against rules 1, 2 and 4 and
refuses one that fails. The embedded copy is tested in CI.
