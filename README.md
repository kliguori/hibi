# hibi

A terminal immersion logging tracker for language learners.
Log the time you spend watching, listening, and reading in your target language, then see it broken down over rolling time windows — no database, just JSON on disk.

`hibi` (日々, "day after day") is built around the immersion workflow: pick what you're immersing in, track the minutes, and watch the streak grow.

## Features

- **Fast logging** — `hibi log` for a quick entry, or a live `hibi clock` stopwatch (with pause/resume) for hands-off timing.
- **A stats dashboard** — totals over today / yesterday / last 7 / 30 / 90 / 360 days / all time, plus current & longest streak and breakdowns by type, mode, and source.
- **Per-language datasets** — track several languages independently, each with its own data and backups; switch with `--lang` or `hibi language use`.
- **Git-backed history** — every change is automatically committed to a git repo in the data directory; browse the full history and restore any past state interactively with `hibi backup`.
- **Fuzzy everything** — selecting, removing, and editing all use an [skim](https://github.com/lotabout/skim) fuzzy picker, so you never type names.

## Data model

Four kinds of records, related by id:

| Record    | Fields                                        | Example            |
|-----------|-----------------------------------------------|--------------------|
| `Type`    | name                                          | anime, podcast     |
| `Mode`    | name                                          | watching, reading  |
| `Source`  | name, type                                    | "Terrace House"    |
| `Session` | source, mode, minutes, date                   | 45m watching, today|

A **source** belongs to a **type**; a **session** references a **source** and a **mode**.
Everything is stored per language as a single JSON file.

## Install

**With [Nix](https://nixos.org)** — reproducible, nothing to set up first:

```sh
nix run github:kliguori/hibi -- stats       # run without installing
nix profile install github:kliguori/hibi     # install onto your PATH
```

**As a flake input** (NixOS / home-manager).
Add the input, forward your flake inputs into your modules, then install the package.

```nix
# flake.nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    hibi.url = "github:kliguori/hibi";
    hibi.inputs.nixpkgs.follows = "nixpkgs"; # reuse your nixpkgs
  };

  outputs = { nixpkgs, hibi, ... }@inputs: {
    nixosConfigurations.<host> = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      specialArgs = { inherit inputs; }; # makes `inputs` reachable in modules
      modules = [ ./configuration.nix ];
    };
  };
}
```

Modules don't see flake inputs unless you pass them with `specialArgs` for NixOS modules and `extraSpecialArgs` for home-manager.
Then reference the package, `inputs.hibi.packages.${pkgs.stdenv.hostPlatform.system}.default`, wherever you install packages:

```nix
# configuration.nix (NixOS)
{ pkgs, inputs, ... }: {
  environment.systemPackages = [ inputs.hibi.packages.${pkgs.stdenv.hostPlatform.system}.default ];
}

# home.nix (home-manager)
{ pkgs, inputs, ... }: {
  home.packages = [ inputs.hibi.packages.${pkgs.stdenv.hostPlatform.system}.default ];
}
```

For home-manager run as a NixOS module, forward inputs with `home-manager.extraSpecialArgs = { inherit inputs; };` instead of `specialArgs`.

**With Cargo** — needs a [Rust toolchain](https://rustup.rs):

```sh
cargo install --git https://github.com/kliguori/hibi
```

**From a local checkout:**

```sh
nix build              # ./result/bin/hibi
cargo build --release  # target/release/hibi
```

## Quick start

```sh
hibi                        # first run seeds a read-only 'sample' dataset
hibi stats                  # explore the sample dashboard
hibi language add japanese  # create your own dataset and switch to it

hibi type add anime
hibi mode add watching
hibi source add "One Piece"   # pick the type from a menu
hibi log                      # pick source + mode, enter minutes
hibi stats                    # see it counted
```

## Commands

Run `hibi help` (or `hibi help <command>`) for full usage.
Anything that selects, removes, or edits an existing item opens a fuzzy picker.

**Logging**
- `hibi log` — pick source + mode, enter minutes (dated today)
- `hibi clock in | out | pause | resume | status | cancel` — live stopwatch

**Records** (`type`, `mode`, `source` all share this shape)
- `hibi type add <name>` · `list` · `rm` · `edit`
- `hibi source add <name>` — pick the type from a menu

**Sessions**
- `hibi session list | add | edit | rm`

**Stats**
- `hibi stats` — the dashboard

**Languages & config**
- `hibi language add <name> | list | use <name>`
- `hibi config show`
- `--lang <name>` — run one command against another dataset

**Backups**
- `hibi backup list | restore` — restore snapshots the current state first, so a mistaken restore is itself undoable.

## Storage

| What                | Location                                          |
|---------------------|---------------------------------------------------|
| Config              | `~/.config/hibi/config.json`                      |
| A language's data   | `~/.local/share/hibi/<language>/hibi.json`        |
| Git history         | `~/.local/share/hibi/.git/`                       |
| The active timer    | `~/.local/share/hibi/clock.json` (one, global)    |

The `sample` dataset is seeded on first run and is **read-only** — explore it, but create your own language to start logging.

## Roadmap

- [ ] **Anki integration** — pull known-word counts and the Anki review streak (via AnkiConnect) and include them alongside immersion time in `hibi stats`, so words-known and immersion hours live on one dashboard.
- [ ] Remote backups — push the data git repo to a remote for off-machine safety and cross-device sync.
- [ ] Charts over time (weekly/monthly trend, not just current windows).
- [ ] Goals and reminders (e.g. a daily minutes target).
- [ ] Optional non-interactive flags on `rm`/`edit` for scripting.

## License

See [LICENSE](LICENSE).
