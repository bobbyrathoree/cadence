use rusqlite::Connection;

use crate::models::prompt::CreatePromptRequest;
use crate::services::{playbook_service, prompt_service};

/// Check if the database is empty and seed starter content on first launch.
pub fn seed_if_empty(conn: &Connection) -> rusqlite::Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM prompts WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    if count > 0 {
        return Ok(());
    }

    seed_starter_kit(conn)?;
    Ok(())
}

fn seed_starter_kit(conn: &Connection) -> rusqlite::Result<()> {
    // --- Prompt 1: Code Review Assistant ---
    let _code_review = prompt_service::create_prompt(
        conn,
        CreatePromptRequest {
            title: "Code Review Assistant".to_string(),
            description: Some("System prompt for thorough, constructive code reviews".to_string()),
            content: r#"You are a senior code reviewer. Analyze the code I share with these priorities:

**Correctness**: Identify bugs, logic errors, off-by-one mistakes, and unhandled edge cases. Flag anything that could fail silently.

**Architecture**: Evaluate whether the code fits the broader system design. Call out tight coupling, missing abstractions, or responsibilities that belong elsewhere.

**Readability**: Suggest clearer naming, better function decomposition, and places where a comment would prevent future confusion. Code is read far more than it's written.

**Performance**: Note unnecessary allocations, N+1 queries, blocking calls in async contexts, or anything that won't scale.

**Security**: Watch for injection risks, improper input validation, leaked secrets, and auth gaps.

For each issue, explain *why* it matters and suggest a concrete fix. Distinguish between must-fix items and nice-to-have improvements. If the code is solid, say so — don't invent problems. Be direct but respectful."#.to_string(),
            variant_label: Some("Default".to_string()),
            tags: vec![
                "starter-kit".to_string(),
                "coding".to_string(),
                "model:claude".to_string(),
                "model:gemini".to_string(),
            ],
            is_favorite: true,
        },
    )?;

    // --- Prompt 2: Project Memory Primer ---
    let project_memory = prompt_service::create_prompt(
        conn,
        CreatePromptRequest {
            title: "Project Memory Primer".to_string(),
            description: Some("Session starter that orients the model with your project's mental map".to_string()),
            content: r#"You are resuming work on a project. Here is the current mental map:

**Project**: [Name and one-line description]
**Stack**: [Languages, frameworks, key dependencies]
**Architecture**: [High-level structure — monorepo? microservices? key modules?]

**Current state**:
- What works: [List stable, shipped features]
- What's in progress: [Active branches, half-built features]
- What's broken: [Known bugs, tech debt hotspots]

**Key decisions made**:
- [Decision 1 and why]
- [Decision 2 and why]

**Patterns to follow**:
- [Naming conventions, file organization, error handling approach]
- [Testing strategy, deployment process]

**Context from last session**:
- [What was accomplished]
- [What was left unfinished]
- [Any open questions or blockers]

Use this context to inform all responses. Ask clarifying questions if my request conflicts with the architecture or patterns described here. Don't repeat this back to me — just internalize it and work from it."#.to_string(),
            variant_label: Some("Default".to_string()),
            tags: vec![
                "starter-kit".to_string(),
                "role:primer".to_string(),
                "model:claude".to_string(),
                "model:gemini".to_string(),
            ],
            is_favorite: true,
        },
    )?;

    // --- Prompt 3: Session Operating Prompt (with two variants) ---
    let session_op = prompt_service::create_prompt(
        conn,
        CreatePromptRequest {
            title: "Session Operating Prompt".to_string(),
            description: Some("Sets the session into multi-mode exploration with adaptive thinking".to_string()),
            content: r#"This session runs in multi-mode exploration. Shift between these thinking modes as the conversation demands — don't wait for me to ask:

**Architectural mode**: Zoom out. Consider how this fits the system. Identify coupling, missing abstractions, and structural consequences of the current approach.

**Diagnostic mode**: Zoom in. Trace the problem methodically. Reproduce the reasoning, check assumptions, find the root cause before proposing fixes.

**Creative mode**: Explore lateral solutions. What if we reframed the problem? What would a different stack, pattern, or mental model suggest?

**Gemini collaboration**: When I mention insights from Gemini, integrate them into your analysis. Look for where Gemini's perspective challenges or strengthens your current recommendation. Flag genuine disagreements rather than smoothing them over — the tension is valuable.

Operating rules:
- Name which mode you're in when you shift
- Challenge my framing if it's leading somewhere unproductive
- Surface trade-offs explicitly — don't bury them in caveats
- If you're uncertain, say so and explain what information would resolve it
- Prefer working code over pseudocode, but flag when an implementation choice is debatable"#.to_string(),
            variant_label: Some("With Gemini".to_string()),
            tags: vec![
                "starter-kit".to_string(),
                "role:mode".to_string(),
                "model:claude".to_string(),
                "model:gemini".to_string(),
            ],
            is_favorite: true,
        },
    )?;

    // Add the Solo variant
    prompt_service::add_variant(
        conn,
        &session_op.prompt.id,
        "Solo",
        r#"This session runs in multi-mode exploration. Shift between these thinking modes as the conversation demands — don't wait for me to ask:

**Architectural mode**: Zoom out. Consider how this fits the system. Identify coupling, missing abstractions, and structural consequences of the current approach.

**Diagnostic mode**: Zoom in. Trace the problem methodically. Reproduce the reasoning, check assumptions, find the root cause before proposing fixes.

**Creative mode**: Explore lateral solutions. What if we reframed the problem? What would a different stack, pattern, or mental model suggest?

**Self-critique mode**: After forming a recommendation, argue against it. What are the strongest objections? What would a senior engineer on the team push back on? Only present your conclusion after it survives this scrutiny.

Operating rules:
- Name which mode you're in when you shift
- Challenge my framing if it's leading somewhere unproductive
- Surface trade-offs explicitly — don't bury them in caveats
- If you're uncertain, say so and explain what information would resolve it
- Prefer working code over pseudocode, but flag when an implementation choice is debatable"#,
    )?;

    // --- Prompt 4: Founder Mode ---
    let founder_mode = prompt_service::create_prompt(
        conn,
        CreatePromptRequest {
            title: "Founder Mode".to_string(),
            description: Some("Step back from tactics and think about what the project could become".to_string()),
            content: r#"Shift into founder mode. Stop thinking about the current ticket and think about the project as a whole.

**Where are we?** Summarize the project's current state honestly — not the roadmap, the reality. What's actually built, what's half-built, what's duct-taped together.

**What's working?** What parts of this project have real momentum? Where does the architecture feel right, the UX feel natural, the code feel clean? Build on these.

**What's not working?** Where are we fighting the design? What keeps coming up as friction — repeated refactors, confusing flows, features that don't land? Be blunt.

**What could this become?** If we stepped back from the current plan and asked "what's the most valuable thing this project could be in 6 months?" — what's the answer? Think beyond the feature list.

**What should we stop doing?** Every project accumulates scope that made sense at the time but doesn't anymore. What should we kill, simplify, or defer indefinitely?

**What's the one thing?** If we could only ship one thing next, what would create the most leverage? Not the most urgent — the most important.

Don't be precious about what's already built. Sunk cost is not a reason to keep going. Think like someone who just acquired this project and is deciding what to do with it."#.to_string(),
            variant_label: Some("Default".to_string()),
            tags: vec![
                "starter-kit".to_string(),
                "role:mode".to_string(),
                "model:claude".to_string(),
                "model:gemini".to_string(),
            ],
            is_favorite: false,
        },
    )?;

    // --- Prompt 5: Session Knowledge Transfer ---
    let knowledge_transfer = prompt_service::create_prompt(
        conn,
        CreatePromptRequest {
            title: "Session Knowledge Transfer".to_string(),
            description: Some("End-of-session handoff that produces a concise map for the next session".to_string()),
            content: r#"This session is ending. Produce a knowledge transfer document that will orient the next session (which may be a different model or a fresh context window).

**Session summary**: What did we work on? What was the goal and did we achieve it?

**Decisions made**:
- [Decision]: [Rationale] — flag if this was a close call or if new information could change it

**Code changes**:
- [File/module]: [What changed and why]
- Note any changes that are incomplete or need follow-up

**Problems encountered**:
- [Problem]: [How we resolved it, or why it's still open]

**Architecture insights**: Any structural observations that emerged — patterns that worked, abstractions that should be extracted, coupling that needs attention.

**Open threads**:
- [Question or task]: [Current thinking, what's needed to resolve]

**Recommended next steps** (in priority order):
1. [Most important next action]
2. [Second priority]
3. [Third priority]

**Warnings**: Anything the next session should watch out for — fragile code, assumptions that might not hold, deadlines, dependencies.

Keep it concise. This is a handoff document, not a narrative. Optimize for someone (or something) that needs to get up to speed in 30 seconds."#.to_string(),
            variant_label: Some("Default".to_string()),
            tags: vec![
                "starter-kit".to_string(),
                "role:closer".to_string(),
                "model:claude".to_string(),
                "model:gemini".to_string(),
            ],
            is_favorite: true,
        },
    )?;

    // --- Prompt 6: Welcome to Cadence ---
    let _welcome = prompt_service::create_prompt(
        conn,
        CreatePromptRequest {
            title: "Welcome to Cadence".to_string(),
            description: Some("A quick tour of how Cadence works".to_string()),
            content: r#"Welcome to Cadence — your prompt library.

This isn't just a note-taking app. Cadence is built for developers who use AI models daily and want to treat their prompts like reusable tools, not throwaway text.

**How it works**:

Each prompt lives as a card with a title, content, and tags. Copy any prompt to your clipboard with one click, then paste it into your AI session.

**Tags** are how you organize and find things. Use them freely:
- `model:claude`, `model:gemini` — mark which models a prompt works well with
- `role:primer`, `role:mode`, `role:closer` — categorize by function in your workflow
- `coding`, `writing`, `planning` — topic tags, whatever makes sense for you

**Search** (`Cmd+Shift+P`) opens the floating search window. It searches titles, content, and tags — so well-tagged prompts surface instantly.

**Variants** let one prompt hold multiple versions. The "Session Operating Prompt" in your starter kit has two variants: one for collaborative sessions with Gemini, one for solo work. Switch between them without duplicating the prompt.

**Playbooks** chain prompts into workflows. The "AI Dev Session Workflow" playbook walks you through a complete coding session — from orienting the model, to choosing a focus, to capturing knowledge at the end.

**Favorites** (star icon) pin prompts to the top of your library for fast access.

Start by exploring the starter prompts, then edit them to match your projects. Delete what you don't need, duplicate what you do. This is your library — make it yours."#.to_string(),
            variant_label: Some("Default".to_string()),
            tags: vec![
                "starter-kit".to_string(),
                "getting-started".to_string(),
            ],
            is_favorite: true,
        },
    )?;

    // --- Starter Playbook: AI Dev Session Workflow ---
    let playbook = playbook_service::create_playbook(
        conn,
        "AI Dev Session Workflow",
        Some("A complete AI-assisted development session from orientation to knowledge capture"),
    )?;

    // Step 1: Project Memory Primer
    playbook_service::add_step(
        conn,
        &playbook.id,
        Some(&project_memory.prompt.id),
        "single",
        Some("Start here. This orients the model with your project's architecture and codebase."),
        None,
    )?;

    // Step 2: Session Operating Prompt
    playbook_service::add_step(
        conn,
        &playbook.id,
        Some(&session_op.prompt.id),
        "single",
        Some("Sets the session mode. The model will shift between architectural, diagnostic, and creative thinking."),
        None,
    )?;

    // Step 3: Founder Mode (any mode prompt)
    playbook_service::add_step(
        conn,
        &playbook.id,
        Some(&founder_mode.prompt.id),
        "single",
        Some("Choose a focus area for this session. You can swap this step with any mode prompt from your library."),
        None,
    )?;

    // Step 4: Session Knowledge Transfer
    playbook_service::add_step(
        conn,
        &playbook.id,
        Some(&knowledge_transfer.prompt.id),
        "single",
        Some("Always end with this. It captures insights and creates a handoff for your next session."),
        None,
    )?;

    Ok(())
}
