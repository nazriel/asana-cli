# Shell completion

`asana setup` installs static command, option, and enum-value completion generated from the CLI's Clap definition. It does not contact Asana or require authentication.

The current shell is inferred from `$SHELL`. Use `--shell bash`, `--shell zsh`, or `--shell fish` to override detection.

| Shell | Default path |
| --- | --- |
| Bash | `${XDG_DATA_HOME:-~/.local/share}/bash-completion/completions/asana` |
| Zsh | `${XDG_DATA_HOME:-~/.local/share}/zsh/site-functions/_asana` |
| Fish | `${XDG_CONFIG_HOME:-~/.config}/fish/completions/asana.fish` |

The command creates missing directories and safely replaces an earlier generated file. It never edits shell startup files. Bash and Fish users can open a new shell or source the installed file. Zsh users must ensure the reported `site-functions` directory is present in `fpath` and that `compinit` is enabled.

Run `asana setup` again after upgrading the CLI so completion includes newly added commands and flags. Use `asana --json setup` when the installation result needs to be consumed programmatically.
