---
description: "Increment patch version, commit, push, and tag the release"
user-invocable: true
disable-model-invocation: true
argument-hint: "[commit message]"
allowed-tools:
 - any
---
Perform a release of the codemason project by following these steps **in order**:

1. **Read the current version** from the workspace `Cargo.toml` (the `version = "..."` line near the top).
2. **Increment the least-significant version digit** by exactly 1.

   - For a version like `0.1.0-beta.9`, increment the pre-release number → `0.1.0-beta.10`.
   - For a version like `1.2.3`, increment the patch number → `1.2.4`.
3. **Update `Cargo.toml`** with the new version string (the workspace-level `version` field only).
4. **Run `cargo check`** to ensure the version change doesn't break the build.
5. **Run `cargo audit`** to ensure that there are no new security issues introduced, stop if there are security issues..
6. **Update **`CHANGELOG.md` with the new version number and any recent changes.
7. **Stage all modified files** with `git add -A`.
8. **Compose the release message.** When this invocation supplied a commit
   message argument, use it verbatim as the release message. When it supplied
   none, generate the message from the changes being released: run
   `git log <last-tag>..HEAD --oneline` (find `<last-tag>` with
   `git describe --tags --abbrev=0`; fall back to `git log -20 --oneline` when
   there is no earlier tag) and read the `## [Unreleased]` section of
   `CHANGELOG.md`, then summarise them into one line.
   - A generated message must be **at most 256 characters**, plain text, one
     line, no trailing period, no version prefix (the commit subject already
     carries `Version: <new-version>`). Prefer the few highest-impact changes;
     when the summary would exceed 256 characters, drop the least important
     items instead of truncating a word mid-way. Never exceed the limit.
   - Never generate a message when one was supplied: use the supplied message
     verbatim, even when it is shorter or longer than a summary would be.
9. **Commit** with the message: `Version: <new-version>`, followed on the next
   line by the release message from step 8 when there is one.
10. **Push** to the remote with `git push`.
11. **Tag** the commit with `v<new-version>` (e.g. `v1.0.0`) and push the tag with `git push origin v<new-version>`.
12. **Report** the old version, new version, the release message, and the tag that was pushed.
