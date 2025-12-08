# `pupgrade`

Yet another dependency upgrade tool, this time designed for periodic monolithic
upgrades of all dependencies across a monorepo. (Mainly [this one][VerTX].)

## Why?

Dependabot isn't flexible enough. Renovate configuration is *pain* and keeps
breaking anyway. My [old `update.ts` monstrosity][update.ts] was also fragile
and painful, so this is hopefully at least slightly more robust.

## `pupgrade`?

Portmanteau of pinned + upgrade, as it's only supposed to handle pinned
dependencies. No relation to the dog vitamins, unless you consider dependency
management to be a chore on par with taking vitamins, I suppose?

## License

Licensed under the [Mozilla Public License, version 2.0][mpl].

[update.ts]: https://github.com/wetheredge/VerTX/blob/1e8f34d0dda6ccfc3ef8760a79634f359e224e16/scripts/update.ts
[VerTX]: https://github.com/wetheredge/VerTX
[mpl]: https://mozilla.org/MPL/2.0/
