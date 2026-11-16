# Saving Your Work with Git

Every game project ends up with an experiment that went wrong: you changed
five things, now nothing works, and you do not remember what it looked like
before. **Version control** solves this. GBADE has it built in, using
**git**, the system almost every programmer uses.

## The idea in one minute

Git keeps a history of **snapshots** of your project, called **commits**.
You decide when to take one, and give it a short message:

```text
History
  Add jumping            2 min ago
  Draw the level         yesterday
  Start tracking this project with GBADE
```

Between snapshots, git knows exactly which lines you changed. You can look at
any snapshot, see what it changed, and throw away changes you regret.

A good habit: commit every time something works. "Player moves", "Sound on
jump", "Fix the score". Small commits make a history you can actually use.

## Start tracking a project

New projects are tracked from the start: **Track changes with Git** is ticked
in the New Project dialog. Git records a name and an email in each snapshot;
GBADE fills them in from your git settings if you have any, otherwise type
whatever you like (they are only stored in this project).

For an existing project, open the **Changes** tab at the top of the left
panel and click **Start tracking changes**. GBADE creates the repository, a
`.gitignore` that leaves out `build/` (it is recreated by every build), and a
first snapshot of everything.

## See what changed

The **Changes** tab lists every file that differs from the last commit:

| Letter | Meaning |
|---|---|
| `A` (green) | a new file |
| `M` (yellow) | a modified file |
| `D` (red) | a deleted file |

The same letters and colours appear next to file names in the **Files** tab.
Click a file to see its changes: removed lines in red, added lines in green.

Inside the editor, the margin shows changed lines as you type: a **green bar**
for new lines, a **blue bar** for modified ones, a **small red triangle**
where lines were deleted.

## Take a snapshot

Write a message under the list ("Add jumping") and click **Commit all**. All
your changes are saved in one snapshot and the list empties. Unsaved files
are saved first.

## Look back

The **History** list shows the latest commits, newest first. Click one to see
what it changed, file by file.

## Undo the changes to a file

Right-click a file in the Changes tab and choose **Discard changes**. After a
confirmation, the file goes back to how it was in the last commit (a new file
is deleted). This cannot be undone, so read the diff first.

## What GBADE does not do (yet)

GBADE's git support works on your computer only. To **share** a project or
keep a copy online (GitHub, GitLab, Codeberg), use any git tool alongside
GBADE: the `git` command line, GitHub Desktop, or your code editor. They see
the same repository, because it is a normal git repository in the `.git`
folder of your project. Branches, merging and pushing are done there.

## Under the hood

GBADE uses **libgit2**, a git library compiled into the app, so no git
installation is required. Your name and email are saved in the project's own
`.git/config`; GBADE never changes your global git settings.
