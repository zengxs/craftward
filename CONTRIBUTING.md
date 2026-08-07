# Contributing to Craftward

## Commit Messages

New commit messages must follow
[Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/):

```text
<type>[optional scope][!]: <description>

[optional body]

[optional footer(s)]
```

- Use one of these types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`,
  `build`, `ci`, `chore`, `style`, or `revert`.
- Use a scope only when it names a stable, recognizable part of the project.
- Write the description in Standard English as a concise imperative phrase,
  without a trailing period.
- Use a body when the motivation, behavior change, or important trade-off is not
  clear from the description alone.
- Mark a breaking change with `!` before the colon or a `BREAKING CHANGE:`
  footer, and explain the impact in the body or footer.
- Keep each commit focused on one coherent change.
