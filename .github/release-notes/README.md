# Release notes

One file per version: `v<version>.md` becomes that GitHub release's body
verbatim. Ship the file **with the version-bump commit** — the workflow
checks out the tag, so the file must exist at the tag to be used.

Language: English only — even though the app's primary audience
is Polish, the releases page is public and the v1.5.0/v1.5.1 notes
set the English precedent.

Style (see `v1.6.1.md`): lead with what the release is about, then
`##` sections by theme (Memory / CPU / Fixes / Install) or
Highlights / Fixes for feature rounds. Concrete numbers over adjectives;
one bullet per user-visible change; the installer story at the bottom.

If a version has no file here, the workflow falls back to GitHub's
generated notes — the old "see the commit history" placeholder is gone.
