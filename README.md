# ghdepup (git hub dep up), the four four time dependency updater

A minimal tool to update dependencies based on tags published on github that is easy to integrate in any kind of toolchain.

For a more universal tool -- e.g. sourcing repositories outside of github -- see e.g.:

* https://github.com/dependabot
* https://github.com/renovatebot/renovate

## project goals

This project aims to provide and update information on tags and releases on github in a way that is easy to integrate into most toolchains and environments. As such it outputs and is configured with a subset of the syntax of [toml](https://github.com/toml-lang/toml) in UTF-8 encoding. So these files should parse as:
* a [POSIX sh](https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html) environment setup script
* a form of minimal [INI file](https://en.wikipedia.org/wiki/INI_file) (using double quoted values)
* a form of [Makefile](https://en.wikipedia.org/wiki/Make_(software)) setting up variables

In addition, the project should:
* stay as much as possible in simple, rust-only build environment
* enable updates via e.g. github actions to create (and possibly automerge) PRs for dependency updates.

`ghdepup` aims to become self-hosting at some point, that is: it should be able to update its own dependencies. While as of now all those are in `Cargo.toml` and thus can be handled by `cargo update`, this is still considered worthwhile to prove the concept.

`ghdepup` is currently build with dynamic linking. At some point, it might be worthwhile to try to make it statically linked, but the implications -- especially for licensing -- have to be carefully considered.

## project non-goals

The project does NOT want to:
* read, write or update a wide set of dependency declarations using in many toolchains. If using ghdepup, the toolchain should be adapted to read and write its files as they are very simple.
* extend sourcing beyond github tags. For open source projects, anything not available on github should be easy to mirror there, thus allowing to use a simple ghdepup tooling.
* extending understanding versioning beyond [semver](https://semver.org/)
* stay simple and minimal, thus making it easy to base other work on it (e.g. using tags on gitlab or forejo/codeberg).

## usage

To update the dependencies of a project, set `GITHUB_TOKEN` in the environment with an github API token, and execute the following command:

    ./ghdepup ghdeps1.toml ghdeps2.toml ghdeps3.toml [...] ghversions.toml

in this, there can be any number bigger than zero of config files declaring dependencies like `ghconfig1.toml` here. For each dependency they contain a block:

    HYPER_GH_PROJECT="hyperium/hyper"
    HYPER_GH_TAG_PREFIX="v"
    HYPER_GH_VERSION_REQ=">=0.14, <1"

In this `HYPER` is the name of this dependency and:
* `HYPER_GH_PROJECT` sets where to find the owner and repository on of the dependency. It is required.
* `HYPER_GH_TAG_PREFIX` sets a possible prefix the tag have before the semantic version. It is not required and assumed to be the empty string when missing.
* `HYPER_GH_VERSION_REQ` sets a version requirement as per [semver](https://semver.org) to restrict the tags/versions to be considered. It is not required and assumed empty when missing.

The last file given as a command -- `ghversions.toml` in the example above should contain the currently used versions of each dependency, e.g.:

    HYPER_GH_VERSION="0.14.26"

This is not required, but the file has to exist. After parsing all these files, `ghdepup` will look for the newest/best version of each dependency, and replace the file with the name given in the last argument -- `ghversions.toml` in the example -- with its findings. From there it can be picked up by the toolchain or build environment.

## features

Some debugging can be enabled by toogling features to `cargo` in the build. They might be described here later.

## self updating

Instead of using `cargo update`, ghdepup updates itself via ghdepup, see `.github/workflow/selfupdate.yml`. For most projects this would be pointless, but here it is just to test ghdepup itself.

## licensing

See LICENSE file in the root directory of the repository.
