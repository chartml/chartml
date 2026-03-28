# The Future of Data & Analytics: Hub-and-Spoke Model

## The Big Picture

The dominant user interface is shifting. Just like DOS gave way to Windows, desktop apps gave way to the browser, and the web gave way to mobile OS — the next platform shift is to **LLM providers as the gateway**.

- **DOS/Windows** — applications ran inside the OS
- **Browser** — apps ran inside Chrome
- **iOS/Android** — the web became apps inside the mobile OS
- **LLM providers** — apps become MCP spokes inside Claude/ChatGPT/Gemini

Not everyone can be Windows, iOS, or Chrome. The major LLM providers (Anthropic, OpenAI, Google) will own the hub. Everyone else builds spokes.

## What This Means for Data & Analytics

**Nobody opens their analytics app.** The LLM is where people do their thinking. Analytics is a subset of thinking. The question becomes the interface. The LLM is the question layer. The analytics spoke is the answer engine behind it.

The analytics app as a destination is the equivalent of Encarta before Google. Google didn't kill Encarta by being a better encyclopedia — it killed it by making the question the interface, not the tool.

## The Spoke's Job

The spoke handles what the hub can't:

- **Data connections** — the persistent, authenticated relationship with the warehouse
- **Semantic understanding** — knowing what "revenue" means in YOUR business
- **Execution** — running queries, caching results, managing freshness
- **Visualization** — rendering rich, interactive charts the hub can't produce
- **Persistence** — always-on monitoring, accumulated knowledge, state between sessions
- **Governance** — permissions, audit trails, who can see what

The spoke app still exists but it's for **power users and configuration** — data engineers setting up connections, analysts defining canonical artifacts, admins managing permissions. Think AWS Console: most people use AWS through code and APIs, but someone still needs the console to configure things.

## The Consistency Problem

**The core challenge:** Two different users asking the same question in different chat sessions can get different results. Dashboards solve this today by being a single source of truth — same query, same chart, same number for everyone. But dashboards are a destination-based artifact from the old paradigm.

**What a dashboard actually is:** Not a visualization. It's a **frozen query**. When you look at it Monday and Tuesday, you trust the comparison because the exact same SQL ran both times. The methodology didn't drift. The only thing that changed is the data.

**The question:** How do you get dashboard-level consistency in a conversational, non-dashboard world?

## The Four-Layer Trust Stack

The solution is a layered system where each layer adds determinism and each has its own governance authority:

### Layer 1: Natural Language Knowledge

Plain English definitions that anyone can write and the LLM interprets.

> "When anyone asks about revenue, use the orders table. Sum the total column, subtract refunds. Only include completed orders. Exclude test accounts."

- **Governed by:** Data engineering (physical layer), domain teams (business logic)
- **Purpose:** Guides the AI toward correct computations
- **Consistency level:** Good but not deterministic — the AI is guided but still generates SQL

### Layer 2: Canonical SQL

The exact query that computes a metric, frozen and version-controlled. Not a rigid schema — just a pinned query that's been approved as the "right way" to compute something.

- **Governed by:** Domain teams (finance approves the revenue query, marketing approves the MQL query)
- **Purpose:** Eliminates query variance — same SQL every time
- **Consistency level:** Deterministic computation

### Layer 3: Canonical Charts (ChartML)

The standardized visualization for a metric. Not just the right number, but the right way to present it — chart type, axes, formatting, everything.

- **Governed by:** Domain teams, design standards
- **Purpose:** Eliminates presentation variance across users and surfaces
- **Consistency level:** Identical visual output everywhere

### Layer 4: Dashboards / Curated Collections

An approved composition of canonical artifacts. The CEO-blessed arrangement of metrics that represents the official view of the business.

- **Governed by:** Executives, leadership
- **Purpose:** "These are the numbers we stand behind"
- **Consistency level:** Fully locked down — same as looking at the dashboard yourself

### Key Design Principles

- **Layers compose optionally.** Not every metric needs all four layers. Exploratory stuff stays at Layer 1. Board-reported KPIs get all four.
- **Natural language, not rigid schemas.** LLMs don't need structured YAML definitions. A paragraph describing a metric works just as well and is writable by business people.
- **Governance is embedded, not bolted on.** No tickets, no approval workflows, no separate governance tool. A finance person writes a learning, tags it as approved. An analyst pins a query. The CEO approves a dashboard. The governance metadata is just another property on artifacts people are already creating.
- **Lightweight, happens in the background.** The moment it feels like enterprise governance software, you've lost.

## Trust Signals on Every Answer

Every result returned by the AI gets evaluated on its trustworthiness based on structural factors — not the AI's self-assessed confidence.

### Trust Tiers

**Verified** — Canonical SQL from an approved dashboard, full governance chain. This is the number the CEO signed off on. Equivalent of looking at the dashboard yourself.

**Canonical** — Used a pinned, approved query, but not part of an approved dashboard. The computation is locked down.

**Guided** — AI generated the SQL but was guided by knowledge definitions. Probably right, but the exact query hasn't been human-approved.

**Generated** — AI wrote the SQL from scratch. No canonical query, no knowledge definition. Treat as exploratory, not authoritative.

### Why This Works

It mirrors trust signals people already understand:

- Wikipedia's "citation needed" tags
- Browser padlock icons for HTTPS
- Financial statements marked "audited" vs "unaudited"

The user doesn't need to understand the governance chain. They see a signal. Green checkmark = "same number your CFO would give you." No indicator = "the AI figured this out on its own, verify before putting it in a deck."

### The Incentive Loop

Trust tiers create a natural flywheel:

1. User asks a question, gets a **Generated** answer
2. They verify it, realize it's useful, pin the query → now it's **Canonical**
3. Finance reviews and approves it → still **Canonical** but with governance
4. It gets added to the KPI dashboard, CEO approves → now it's **Verified**

The org's trust coverage grows organically from the bottom up. No big governance project needed. Things that matter get progressively locked down because users naturally want their important numbers to carry the trust signal.

### What the Hub Does With Trust Tiers

The hub uses trust contextually:

- **Board prep:** "3 of these 5 metrics are Generated, not Verified. Want me to flag those for review before you present?"
- **Casual exploration:** Trust tier shown but no friction
- **Regulated context:** Spoke configured to only return Verified answers; everything else gets "I don't have an approved answer for that — here's an exploratory result, contact the finance team to make it official"

### The Subtle Difference

The spoke doesn't say "I'm not confident in this answer" — that's the AI hedging, which erodes trust in everything. Instead it says "this computation hasn't been reviewed by finance yet." This shifts trust from "do I trust the AI" to "do I trust my org's data governance" — a question people already know how to answer.

## Canonical Charts as Composable Building Blocks

Dashboards today are static arrangements of charts. In the spoke model, canonical charts become composable components that can appear anywhere:

- In a Claude conversation when someone asks a question
- In a Slack message when a watch fires
- In an email digest
- Assembled into a dashboard for people who want that view
- Embedded in a Google Doc or Notion page
- Combined with other canonical charts on the fly

The hub says "compare revenue to churn for the last 4 quarters" — the spoke pulls two canonical charts and renders them side by side. Nobody built that dashboard. It assembled itself from canonical components.

ChartML is already built for this — declarative, portable, renderable anywhere. It just needs the registry of canonical specs behind it.

## The Competitive Moat

A competitor can spin up a SQL engine and connect to the same warehouse. They can't replicate:

- Months of accumulated knowledge definitions
- The approved queries finance signed off on
- The canonical charts the team standardized on
- The dashboards the CEO blessed
- The full governance chain connecting all of it

That's institutional trust, encoded in the spoke. It took months to build and can't be exported to a CSV.

The spoke that accumulates the deepest understanding of an organization's data wins. Raw data access is commoditized. Understanding the data is the moat.

## Where Investment Goes

**Build:**
- Semantic knowledge base with great retrieval (evolved learnings system)
- Canonical SQL pinning and versioning
- Canonical ChartML registry
- Trust tier computation and display
- Conflict detection (flag when two queries answer the same question differently)
- Ownership and lightweight governance metadata

**Maintain but don't over-invest:**
- Dashboard UI (becomes one rendering surface among many, not the primary product)
- Chat interface (the hub does this better)

**Don't build:**
- Rigid metric schemas / YAML configurations — LLMs don't need them
- Enterprise governance workflows — keep it lightweight
- Query editors for casual users — the hub handles natural language

---

*Brainstormed: 2026-02-07*
