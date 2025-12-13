use super::{HelpBook, HelpFlag, HelpTopic, Section};

pub(crate) fn book() -> HelpBook<'static> {
    HelpBook {
        title: "Quick Notes CLI",
        usage: "qn <command> [options]",
        topics: ALL_TOPICS,
        footer: &[
            "Use `qn help <topic>` for focused docs, e.g. `qn help list` or `qn help tags`.",
            "Help text lives outside the rendering code so other binaries can reuse it.",
        ],
    }
}

const ALL_TOPICS: &[HelpTopic<'static>] = &[
    HelpTopic {
        name: "add",
        summary: "Append text to an existing note by id.",
        usage: "qn add <id> \"text\"",
        details: &[
            "Reads the note body, appends the provided text (plus a trailing newline), and bumps the Updated header.",
            "IDs can be picked quickly via shell completion; errors if the id is missing.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Command,
        examples: &["qn add 07Dec25-115301 \"extra context\""],
    },
    HelpTopic {
        name: "new",
        summary: "Create a note with a title, optional body, and tags.",
        usage: "qn new <title> [body...] [-t tag...]",
        details: &[
            "Generates a microsecond-based id, writes the Markdown header, and stores normalized tags.",
            "Body text after the title is joined with spaces; tags can be repeated to add several.",
        ],
        flags: &[HelpFlag {
            name: "-t, --tag <tag>",
            desc: "Attach a tag; normalization turns \"todo\" into \"#todo\".",
        }],
        aliases: &[],
        section: Section::Command,
        examples: &["qn new \"Project brief\" first draft -t #work -t todo"],
    },
    HelpTopic {
        name: "list",
        summary: "List notes with previews; sorted by updated desc by default.",
        usage: "qn list [--sort created|updated|size] [--asc|--desc] [-s text] [-t tag] [--relative|-r] [--all|-a]",
        details: &[
            "Matches search text against title and body (case-insensitive).",
            "Tag filters accept normalized tags; multiple tags require that all are present.",
        ],
        flags: &[
            HelpFlag {
                name: "--sort <field>",
                desc: "created|updated|size (default updated)",
            },
            HelpFlag {
                name: "--asc / --desc",
                desc: "Ascending or descending sort (default desc).",
            },
            HelpFlag {
                name: "-s, --search <text>",
                desc: "Substring search against title and body.",
            },
            HelpFlag {
                name: "-t, --tag <tag>",
                desc: "Filter by tag (normalized to #tag).",
            },
            HelpFlag {
                name: "--relative, -r",
                desc: "Show age instead of absolute timestamps.",
            },
            HelpFlag {
                name: "--all, -a",
                desc: "Disable pagination; show all results.",
            },
        ],
        aliases: &[],
        section: Section::Command,
        examples: &[
            "qn list --sort size --desc",
            "qn list -s meeting -t #todo",
        ],
    },
    HelpTopic {
        name: "list-deleted",
        summary: "List trashed notes with created/updated/deleted columns.",
        usage: "qn list-deleted [--sort created|updated|size] [--asc|--desc] [-s text] [-t tag] [--relative|-r] [--all|-a]",
        details: &[
            "Behaves like list but reads from the trash directory and shows Deleted timestamps.",
            "Old trash entries expire after QUICK_NOTES_TRASH_RETENTION_DAYS (default 30).",
        ],
        flags: &[
            HelpFlag {
                name: "--sort <field>",
                desc: "created|updated|size (default updated)",
            },
            HelpFlag {
                name: "--asc / --desc",
                desc: "Ascending or descending sort (default desc).",
            },
            HelpFlag {
                name: "-s, --search <text>",
                desc: "Substring search against title and body.",
            },
            HelpFlag {
                name: "-t, --tag <tag>",
                desc: "Filter by tag (normalized to #tag).",
            },
            HelpFlag {
                name: "--relative, -r",
                desc: "Show age instead of absolute timestamps.",
            },
            HelpFlag {
                name: "--all, -a",
                desc: "Disable pagination; show all results.",
            },
        ],
        aliases: &[],
        section: Section::Command,
        examples: &["qn list-deleted --sort created --asc"],
    },
    HelpTopic {
        name: "list-archived",
        summary: "List archived notes; shows when each entry was archived.",
        usage: "qn list-archived [--sort created|updated|size] [--asc|--desc] [-s text] [-t tag] [--relative|-r] [--all|-a]",
        details: &[
            "Reads from the archive directory and includes Archived timestamps.",
            "Useful for finding older notes that were tucked away but not deleted.",
        ],
        flags: &[
            HelpFlag {
                name: "--sort <field>",
                desc: "created|updated|size (default updated)",
            },
            HelpFlag {
                name: "--asc / --desc",
                desc: "Ascending or descending sort (default desc).",
            },
            HelpFlag {
                name: "-s, --search <text>",
                desc: "Substring search against title and body.",
            },
            HelpFlag {
                name: "-t, --tag <tag>",
                desc: "Filter by tag (normalized to #tag).",
            },
            HelpFlag {
                name: "--relative, -r",
                desc: "Show age instead of absolute timestamps.",
            },
            HelpFlag {
                name: "--all, -a",
                desc: "Disable pagination; show all results.",
            },
        ],
        aliases: &[],
        section: Section::Command,
        examples: &["qn list-archived -s design -r"],
    },
    HelpTopic {
        name: "view",
        summary: "Render one or more notes; works as `qn view` or `qn render`.",
        usage: "qn view <id>... [--render|-r] [--plain|-p] [-t tag]",
        details: &[
            "Loads each id, enforces optional tag filters, and prints the header plus rendered body.",
            "Uses glow for rich Markdown when available; falls back to internal styling.",
        ],
        flags: &[
            HelpFlag {
                name: "--render, -r",
                desc: "Force Markdown rendering even when invoked as `view`.",
            },
            HelpFlag {
                name: "--plain, -p",
                desc: "Disable colors and formatting.",
            },
            HelpFlag {
                name: "-t, --tag <tag>",
                desc: "Only show notes containing the tag.",
            },
        ],
        aliases: &["render"],
        section: Section::Command,
        examples: &[
            "qn view 20231201-120000 --plain",
            "qn render 20231201-120000 20231201-121500",
        ],
    },
    HelpTopic {
        name: "edit",
        summary: "Open notes in $EDITOR; supports tag guards and fzf multi-select.",
        usage: "qn edit <id>... [-t tag]",
        details: &[
            "When no ids are provided, fzf launches a 70% height picker with previews (unless QUICK_NOTES_NO_FZF is set).",
            "After saving, the Updated header is refreshed; missing tag filters skip the note.",
        ],
        flags: &[HelpFlag {
            name: "-t, --tag <tag>",
            desc: "Require that selected notes contain the tag.",
        }],
        aliases: &[],
        section: Section::Command,
        examples: &["qn edit -t #todo", "QUICK_NOTES_NO_FZF=1 qn edit id1 id2"],
    },
    HelpTopic {
        name: "delete",
        summary: "Soft-delete notes to trash; interactive with fzf when requested.",
        usage: "qn delete [ids...] [--fzf] [-t tag]",
        details: &[
            "Moves files into the trash directory and stamps a Deleted time; trash is cleaned after retention days.",
            "With no ids, `--fzf` (and an installed fzf) opens a multi-select picker with previews.",
        ],
        flags: &[
            HelpFlag {
                name: "--fzf",
                desc: "Launch interactive picker when no ids are given.",
            },
            HelpFlag {
                name: "-t, --tag <tag>",
                desc: "Only delete notes containing the tag.",
            },
        ],
        aliases: &[],
        section: Section::Command,
        examples: &["qn delete --fzf", "qn delete id1 id2 -t #done"],
    },
    HelpTopic {
        name: "delete-all",
        summary: "Move every note in the active area to trash.",
        usage: "qn delete-all",
        details: &[
            "Scans the active directory and moves each note into trash with a Deleted timestamp.",
            "Skipped if no notes exist; retention still applies to trashed files.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Command,
        examples: &["qn delete-all"],
    },
    HelpTopic {
        name: "archive",
        summary: "Move notes to the archive; interactive when fzf is available.",
        usage: "qn archive <ids...> [--fzf]",
        details: &[
            "Archives keep content indefinitely but hide from the active list.",
            "With no ids, requires --fzf and an installed fzf to pick entries.",
        ],
        flags: &[HelpFlag {
            name: "--fzf",
            desc: "Interactive picker when no ids are supplied.",
        }],
        aliases: &[],
        section: Section::Command,
        examples: &["qn archive --fzf", "qn archive id1 id2"],
    },
    HelpTopic {
        name: "unarchive",
        summary: "Restore archived notes to the active area.",
        usage: "qn unarchive <ids...>",
        details: &[
            "Moves files out of archive; name conflicts are resolved by renaming.",
            "Accepts multiple ids; errors when ids are missing.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Command,
        examples: &["qn unarchive id1 id2"],
    },
    HelpTopic {
        name: "undelete",
        summary: "Restore trashed notes back to active storage.",
        usage: "qn undelete <ids...>",
        details: &[
            "Reads from trash, restores timestamps, and renames on conflict.",
            "Use `qn list-deleted` to see candidate ids.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Command,
        examples: &["qn undelete 20231201-120000"],
    },
    HelpTopic {
        name: "migrate-ids",
        summary: "Rewrite filenames to the short incremental id scheme.",
        usage: "qn migrate-ids",
        details: &[
            "Scans current notes, generates new ids, and renames files accordingly.",
            "Skips work if there are no note files to migrate.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Command,
        examples: &["qn migrate-ids"],
    },
    HelpTopic {
        name: "migrate",
        summary: "Import notes from another directory into a migrated batch.",
        usage: "qn migrate <path>",
        details: &[
            "Copies Markdown notes from the provided folder into `~/.quick_notes/migrated/<batch>`.",
            "Keeps Created/Updated headers when present; generates a fresh id if a collision is found.",
            "Migrated notes show up in list/view/edit alongside existing active notes.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Command,
        examples: &["qn migrate ~/Downloads/old_notes"],
    },
    HelpTopic {
        name: "tags",
        summary: "List tags with counts and first/last usage; supports search.",
        usage: "qn tags [-s text] [--relative|-r]",
        details: &[
            "Pinned tags remain visible even if unused (see QUICK_NOTES_PINNED_TAGS).",
            "Relative mode shows age instead of absolute timestamps.",
        ],
        flags: &[
            HelpFlag {
                name: "-s, --search <text>",
                desc: "Filter tag names by substring.",
            },
            HelpFlag {
                name: "--relative, -r",
                desc: "Show ages instead of timestamps for first/last used.",
            },
        ],
        aliases: &[],
        section: Section::Command,
        examples: &["qn tags -s todo", "qn tags -r"],
    },
    HelpTopic {
        name: "seed",
        summary: "Generate bulk test notes with optional markdown bodies.",
        usage: "qn seed <count> [--chars N] [--markdown] [-t tag...]",
        details: &[
            "Creates microsecond ids and random bodies; defaults to 400 characters unless --chars is provided.",
            "When --markdown is set, sample Markdown snippets are used instead of random text.",
        ],
        flags: &[
            HelpFlag {
                name: "--chars <N>",
                desc: "Body length for generated notes (default 400).",
            },
            HelpFlag {
                name: "--markdown",
                desc: "Use Markdown sample bodies instead of random text.",
            },
            HelpFlag {
                name: "-t, --tag <tag>",
                desc: "Attach tags to generated notes (repeatable).",
            },
        ],
        aliases: &[],
        section: Section::Command,
        examples: &["qn seed 50 --chars 120", "qn seed 10 --markdown -t #demo"],
    },
    HelpTopic {
        name: "stats",
        summary: "Show totals for active, trash, and archive areas.",
        usage: "qn stats",
        details: &[
            "Counts notes in each area and prints a small summary table.",
            "Useful for sanity checks after bulk delete/archive operations.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Command,
        examples: &["qn stats"],
    },
    HelpTopic {
        name: "path",
        summary: "Print the notes directory path.",
        usage: "qn path",
        details: &[
            "Respects QUICK_NOTES_DIR when set; defaults to ~/.quick_notes.",
            "Often used by scripts to locate the backing files.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Command,
        examples: &["QUICK_NOTES_DIR=/tmp/notes qn path"],
    },
    HelpTopic {
        name: "completion",
        summary: "Emit the zsh/fzf completion script.",
        usage: "qn completion zsh",
        details: &[
            "Outputs the shell snippet that enables `qn` and `quick_notes` completions with fzf previews.",
            "Source the output in your shell or install it via your plugin manager.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Command,
        examples: &["source <(qn completion zsh)"],
    },
    HelpTopic {
        name: "help",
        summary: "Show the overview or a specific topic.",
        usage: "qn help [topic]",
        details: &[
            "Works like git's help flow: `qn help` shows the overview; `qn help list` drills into one command.",
            "Also available through `qn --help`.",
        ],
        flags: &[],
        aliases: &["--help", "-h"],
        section: Section::Command,
        examples: &["qn help view", "qn --help"],
    },
    HelpTopic {
        name: "getting-started",
        summary: "Fast path to your first notes and search.",
        usage: "qn help getting-started",
        details: &[
            "Create a note with `qn new \"Title\" body...` then list it with `qn list`.",
            "Use tags from the beginning (`-t #todo`) so list/view/edit/delete can filter cleanly.",
            "Render Markdown with `qn render <id>`; use `qn list` first to grab the id.",
        ],
        flags: &[],
        aliases: &["quickstart"],
        section: Section::Guide,
        examples: &[
            "qn new \"Standup\" yesterday blockers -t #team -t #status",
            "qn list --sort created --desc",
            "qn render 20240101-120000",
        ],
    },
    HelpTopic {
        name: "searching",
        summary: "Search and filter strategy for growing notebooks.",
        usage: "qn help searching",
        details: &[
            "Combine substring search (-s) with tags (-t) to narrow quickly; searches hit both title and body.",
            "Favor short, reusable tags (#todo, #meeting, #decision) and pin them via QUICK_NOTES_PINNED_TAGS.",
            "Use archive for long-term storage and list-archived when you need to resurface older work.",
        ],
        flags: &[],
        aliases: &["filtering"],
        section: Section::Guide,
        examples: &[
            "qn list -s auth -t #decision",
            "qn list-archived -s \"2023 roadmap\" -r",
            "QUICK_NOTES_PINNED_TAGS=\"#todo,#decision\" qn tags",
        ],
    },
    HelpTopic {
        name: "bulk-ops",
        summary: "Seed, prune, and archive at scale.",
        usage: "qn help bulk-ops",
        details: &[
            "Use `qn seed <count>` to create load for testing; add --markdown for realistic bodies.",
            "Archive or delete interactively with fzf by omitting ids and passing --fzf (fzf must be installed).",
            "Trash auto-cleans after QUICK_NOTES_TRASH_RETENTION_DAYS, or set it to 0 to disable cleanup.",
        ],
        flags: &[],
        aliases: &["bulk"],
        section: Section::Guide,
        examples: &[
            "qn seed 100 --chars 200 -t #perf",
            "qn archive --fzf",
            "QUICK_NOTES_TRASH_RETENTION_DAYS=7 qn list-deleted",
        ],
    },
    HelpTopic {
        name: "QUICK_NOTES_DIR",
        summary: "Override the notes directory (default ~/.quick_notes).",
        usage: "QUICK_NOTES_DIR=/path qn new ...",
        details: &[
            "Set to point the CLI at a different workspace; affects all commands.",
            "Directory is created on demand if it does not exist.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Environment,
        examples: &["QUICK_NOTES_DIR=/tmp/notes qn list"],
    },
    HelpTopic {
        name: "QUICK_NOTES_TRASH_RETENTION_DAYS",
        summary: "Control how long trashed notes are kept (default 30).",
        usage: "QUICK_NOTES_TRASH_RETENTION_DAYS=60 qn delete ...",
        details: &[
            "Values are interpreted in days; zero disables automatic trash cleanup.",
            "Applied whenever listing or deleting trash so it stays tidy over time.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Environment,
        examples: &["QUICK_NOTES_TRASH_RETENTION_DAYS=7 qn list-deleted"],
    },
    HelpTopic {
        name: "QUICK_NOTES_PINNED_TAGS",
        summary: "Comma-separated list of tags to pin in `qn tags` output.",
        usage: "QUICK_NOTES_PINNED_TAGS=\"#todo,#scratch\" qn tags",
        details: &[
            "Defaults to #todo,#meeting,#scratch; pinned tags remain visible even with zero usage.",
            "Useful for keeping common tags near the top while browsing.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Environment,
        examples: &["QUICK_NOTES_PINNED_TAGS=\"#retro\" qn tags"],
    },
    HelpTopic {
        name: "QUICK_NOTES_NO_FZF",
        summary: "Disable fzf integrations even if fzf is installed.",
        usage: "QUICK_NOTES_NO_FZF=1 qn edit",
        details: &[
            "Forces commands to skip fzf popups; useful in constrained shells or CI.",
            "Affects edit/delete/archive paths that normally launch fzf when no ids are provided.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Environment,
        examples: &["QUICK_NOTES_NO_FZF=1 qn delete id1"],
    },
    HelpTopic {
        name: "NO_COLOR",
        summary: "Disable colored output in render, list, and tags.",
        usage: "NO_COLOR=1 qn list",
        details: &[
            "Honored by rendering, listing, and tag displays; keeps output monochrome for piping.",
        ],
        flags: &[],
        aliases: &[],
        section: Section::Environment,
        examples: &["NO_COLOR=1 qn view 20231201-120000"],
    },
];
