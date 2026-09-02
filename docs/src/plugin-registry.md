# Plugin Registry

`mantis` supports a **git-backed plugin registry** for discovering and installing
third-party plugins. The registry is a plain git repository containing an
`index.json` file that lists available plugins. No HTTP library is used — `mantis`
uses `git clone` and `git pull` to sync the registry.

## How it works

1. On first access, `mantis` clones the registry repository into the local cache at
   `~/.config/mantis/registry/`.
2. On subsequent access, `mantis` refreshes the cache from the pinned `main`
   branch with `git pull --ff-only`.
3. The `index.json` file is parsed to build the list of available plugins.

Registry refreshes that cannot be fast-forwarded are recovered by cloning a
fresh `main` checkout and replacing the cache only after the clone contains a
valid `index.json`. If recovery fails, the previous cache is preserved when
possible and the error is reported to the caller.

Bundled plugins (such as the built-in `python` language provider at
`plugins/python/`) are not listed in the registry — they ship with `mantis`
and are installed automatically at startup. See
[Plugin Development](plugin-development.md) for how bundled plugins are
structured.

## Default registry

The default registry is hosted at:

```
https://github.com/ansromanov/mantis-plugins
```

To use a different registry, set the `MANTIS_PLUGIN_REGISTRY` environment variable:

```bash
export MANTIS_PLUGIN_REGISTRY="https://github.com/your-org/your-plugin-registry"
```

## Cache location

The registry cache lives at:

| Platform | Path |
|---|---|
| Linux / macOS | `~/.config/mantis/registry/` (or `$XDG_CONFIG_HOME/mantis/registry/`) |
| Windows | `%APPDATA%\mantis\registry\` |

Override with the `MANTIS_PLUGIN_REGISTRY_DIR` environment variable:

```bash
export MANTIS_PLUGIN_REGISTRY_DIR="/custom/path/to/registry"
```

## index.json format

The registry repository must contain an `index.json` file at its root with the
following structure:

```json
{
  "plugins": [
    {
      "name": "git-tools",
      "description": "git diff/log integration",
      "repo": "https://github.com/example/tv-git-tools",
      "tag": "v0.1.0",
      "sha256": "<64 lowercase hexadecimal characters>"
    },
    {
      "name": "markdown-preview",
      "description": "Live markdown preview panel",
      "repo": "https://github.com/example/tv-md-preview",
      "tag": "v0.2.0",
      "sha256": "<64 lowercase hexadecimal characters>"
    }
  ]
}
```

### Fields

| Field | Required | Description |
|---|---|---|
| `name` | Yes | Unique plugin name (used for resolution). |
| `description` | Yes | One-line description shown in search results. |
| `repo` | Yes | Git repository URL (HTTPS or SSH). |
| `tag` | Yes | Git tag or branch to check out when installing. |
| `sha256` | Required for installation | Lowercase hexadecimal SHA-256 digest of the released artifact. Legacy entries may omit it for discovery, but installation must reject them. |

## Trust and consent

Registry entries are metadata only; a plugin must be verified before its
artifact is installed. The host compares the downloaded artifact with the
entry's `sha256` value and refuses a mismatch or a missing digest.

Discovered third-party plugin manifests are disabled by default. Discovery
does not execute plugin code; the user must explicitly enable a plugin from
the plugin manager or configuration. The manifest's `permissions` field is
advisory and describes requested access for review—it is not a sandbox.

## Searching

Use the plugin picker in `mantis` (accessible via the plugin overlay) to search the
registry by name or description. The search is case-insensitive and matches
substrings in both `name` and `description` fields.

## Resolution

To look up a specific plugin by name (e.g. for installation), `mantis` uses exact
case-sensitive matching against the `name` field in `index.json`.

## Hosting your own registry

To create your own plugin registry:

1. Create a new git repository.
2. Add an `index.json` file following the format above.
3. Push to a git remote (GitHub, GitLab, self-hosted, etc.).
4. Set `MANTIS_PLUGIN_REGISTRY` to your repository URL.

Example — create a minimal registry:

```bash
mkdir my-plugin-registry
cd my-plugin-registry
git init
cat > index.json << 'EOF'
{
  "plugins": [
    {
      "name": "my-plugin",
      "description": "My custom mantis plugin",
      "repo": "https://github.com/me/mantis-my-plugin",
      "tag": "v1.0.0"
    }
  ]
}
EOF
git add index.json
git commit -m "initial registry"
git remote add origin https://github.com/me/my-plugin-registry
git push -u origin main
```

Then point `mantis` at it:

```bash
export MANTIS_PLUGIN_REGISTRY="https://github.com/me/my-plugin-registry"
```

> **Tip:** Make sure the repository is accessible without authentication for
> `mantis` to clone it. Public GitHub repos work out of the box.
