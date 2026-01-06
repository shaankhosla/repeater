# Installation

Pick the method that matches your platform or workflow.

## Install Script (Linux & macOS)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/shaankhosla/repeater/releases/latest/download/repeater-installer.sh | sh
```

## Homebrew (macOS)

```sh
brew install shaankhosla/tap/repeater
```

## Windows (PowerShell)

```powershell
irm https://github.com/shaankhosla/repeater/releases/latest/download/repeater-installer.ps1 | iex
```

## npm

```sh
npm install @shaankhosla/repeater
```

## Optional: add a `rpt` shortcut

Use `repeater` in docs and scripts so examples stay canonical. If you prefer a shorter command locally, add `rpt` with one of these snippets.

### macOS / Linux

#### Bash (`~/.bashrc` or `~/.bash_profile`)

```sh
alias rpt='repeater'
```

Reload:

```sh
source ~/.bashrc  # or: source ~/.bash_profile
```

#### Zsh (`~/.zshrc`)

```sh
alias rpt='repeater'
```

Reload:

```sh
source ~/.zshrc
```

#### Fish (`~/.config/fish/config.fish`)

```fish
alias rpt repeater
```

Reload:

```fish
source ~/.config/fish/config.fish
```

#### Optional: symlink instead of alias (any shell)

```sh
ln -s "$(command -v repeater)" /usr/local/bin/rpt
```

On Apple Silicon you may prefer `/opt/homebrew/bin`; add `sudo` if permissions require it.

### Windows

#### PowerShell (profile)

```powershell
notepad $PROFILE
```

Add this line, save, and restart PowerShell:

```powershell
Set-Alias rpt repeater
```

#### Command Prompt (per session)

```bat
doskey rpt=repeater $*
```

#### Optional: `rpt.cmd` shim (permanent)

Create `rpt.cmd` on your PATH containing:

```bat
@echo off
repeater %*
```
