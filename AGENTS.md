# Agent workflow

Use [CONTRIBUTING.md](CONTRIBUTING.md) for setup and checks. GitHub issues own active
work and acceptance criteria.

- Put enforceable invariants in types, validation, or tests. Documentation must not
  substitute for enforcement or repeat what the code already explains.
- Keep Markdown for a reader's task, usage, or necessary attribution. Do not add
  decision diaries, speculative design inventories, or copies of the issue tracker.
- At the end of every implementation pass, spawn a gpt-6.1-sol subagent with xhigh
  reasoning to review maintainability, readability, and performance. Address substantive
  findings and verify fixes before handoff. If unavailable, report the limitation.
