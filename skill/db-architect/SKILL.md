# DB Architect Skill

Use this skill when designing relational schemas and query plans, especially for hierarchical data.

## Current Focus

For now, prioritize one problem:
- store hierarchical data over relational storage,
- read complete/partial conversation trees efficiently.

## Baseline Modeling Strategy

1. `conversations` table for thread/session root metadata.
2. `messages` table with adjacency link:
   - `id`, `conversation_id`, `parent_id`, `role`, `content`, `created_at`.
3. `message_closure` table for scalable hierarchy reads:
   - `ancestor_id`, `descendant_id`, `depth`.

Use adjacency + closure by default for production read-heavy paths.

## Baseline Queries

Full conversation timeline:

```sql
select m.*
from messages m
where m.conversation_id = $1
order by m.created_at asc, m.id asc;
```

Subtree from a node:

```sql
select m.*
from message_closure c
join messages m on m.id = c.descendant_id
where c.ancestor_id = $1
order by c.depth asc, m.created_at asc, m.id asc;
```

Ancestor chain for a node:

```sql
select m.*
from message_closure c
join messages m on m.id = c.ancestor_id
where c.descendant_id = $1
order by c.depth desc, m.created_at asc, m.id asc;
```

## Index Baseline

- `messages(conversation_id, created_at, id)`
- `messages(parent_id)`
- `message_closure(ancestor_id, depth, descendant_id)`
- `message_closure(descendant_id, depth, ancestor_id)`

## Execution Rules

1. Default to transactionally maintaining closure rows on message insert.
2. If closure is not acceptable, provide recursive CTE fallback and performance caveats.
3. Always provide deterministic ordering in read queries.
4. Call out write amplification/read latency tradeoff explicitly.

## Output Expectations

- DDL for core tables and indexes
- write path strategy (insert + closure maintenance)
- read queries for timeline, subtree, and ancestry
- tradeoffs and scaling notes
