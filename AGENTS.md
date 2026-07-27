# finima

> Multi-agent orchestration framework for agentic coding

## Project Overview

A Claude Flow powered project

**Tech Stack**: TypeScript, Node.js
**Architecture**: Domain-Driven Design with bounded contexts

## Quick Start

### Installation

```bash
npm install
```

### Build

```bash
npm run build
```

### Test

```bash
npm test
```

### Development

```bash
npm run dev
```

## Agent Coordination

### Swarm Configuration

This project uses hierarchical swarm coordination for complex tasks:

| Setting    | Value          | Purpose                             |
| ---------- | -------------- | ----------------------------------- |
| Topology   | `hierarchical` | Queen-led coordination (anti-drift) |
| Max Agents | 8              | Optimal team size                   |
| Strategy   | `specialized`  | Clear role boundaries               |
| Consensus  | `raft`         | Leader-based consistency            |

### When to Use Swarms

**Invoke swarm for:**

- Multi-file changes (3+ files)
- New feature implementation
- Cross-module refactoring
- API changes with tests
- Security-related changes
- Performance optimization

**Skip swarm for:**

- Single file edits
- Simple bug fixes (1-2 lines)
- Documentation updates
- Configuration changes

### Available Skills

Use `$skill-name` syntax to invoke:

| Skill                  | Use Case                            |
| ---------------------- | ----------------------------------- |
| `$swarm-orchestration` | Multi-agent task coordination       |
| `$memory-management`   | Pattern storage and retrieval       |
| `$sparc-methodology`   | Structured development workflow     |
| `$security-audit`      | Security scanning and CVE detection |

### Agent Types

| Type         | Role                  | Use Case             |
| ------------ | --------------------- | -------------------- |
| `researcher` | Requirements analysis | Understanding scope  |
| `architect`  | System design         | Planning structure   |
| `coder`      | Implementation        | Writing code         |
| `tester`     | Test creation         | Quality assurance    |
| `reviewer`   | Code review           | Security and quality |

## Execution Model

- **claude-flow** = LEDGER (coordinates: memory, routing, swarm state)
- **Codex** = EXECUTOR (writes code, runs tests, creates files)

**Critical rule:** DON'T STOP after calling claude-flow commands. Coordination commands return instantly — continue immediately with the next implementation step.

## MCP Integration

Use MCP tools for coordination, then keep coding:

| Tool               | Purpose            | Example                                     |
| ------------------ | ------------------ | ------------------------------------------- |
| `swarm_init`       | Start coordination | `swarm_init({topology: "hierarchical"})`    |
| `memory_store`     | Save patterns      | `memory_store({key: "auth", value: "JWT"})` |
| `memory_search`    | Find patterns      | `memory_search({query: "auth patterns"})`   |
| `task_orchestrate` | Assign work        | `task_orchestrate({task: "implement"})`     |

## Code Standards

### File Organization

- **NEVER** save to root folder
- `/src` - Source code files
- `/tests` - Test files
- `/docs` - Documentation
- `/config` - Configuration files

### Quality Rules

- Files under 500 lines
- No hardcoded secrets
- Input validation at boundaries
- Typed interfaces for public APIs
- TDD London School (mock-first) preferred

### Commit Messages

```text
<type>(<scope>): <description>

[optional body]

Co-Authored-By: claude-flow <ruv@ruv.net>
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`

## Security

### Critical Rules

- NEVER commit secrets, credentials, or .env files
- NEVER hardcode API keys
- Always validate user input
- Use parameterized queries for SQL
- Sanitize output to prevent XSS

### Path Security

- Validate all file paths
- Prevent directory traversal (../)
- Use absolute paths internally

## Memory System

### Storing Patterns

```bash
npx @claude-flow/cli memory store \
  --key "pattern-name" \
  --value "pattern description" \
  --namespace patterns
```

### Searching Memory

```bash
npx @claude-flow/cli memory search \
  --query "search terms" \
  --namespace patterns
```

## Quick Commands

```bash
npx @claude-flow/cli memory search --query "relevant patterns"
npx @claude-flow/cli hooks route --task "current task description"
npx @claude-flow/cli swarm init --topology hierarchical
npx @claude-flow/cli hooks pre-task --description "task summary"
```

## Links

- Documentation: https://github.com/ruvnet/ruflo
- Issues: https://github.com/ruvnet/ruflo/issues

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:7510c1e2 -->

## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:

   ```bash
   git pull --rebase
   git push
   git status  # MUST show "up to date with origin"
   ```

5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**

- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

<!-- END BEADS INTEGRATION -->
