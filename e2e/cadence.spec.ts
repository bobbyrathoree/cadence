import { expect, test, type Page } from "@playwright/test";

const PINNED_CSP =
  "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src ipc: http://ipc.localhost";

const cspViolations = new WeakMap<Page, string[]>();

test.beforeEach(async ({ page }) => {
  const violations: string[] = [];
  cspViolations.set(page, violations);
  page.on("console", (message) => {
    const text = message.text();
    if (
      /content security policy/i.test(text) ||
      /violates the following.*directive/i.test(text)
    ) {
      violations.push(text);
    }
  });
  await page.addInitScript(() => {
    window.addEventListener("securitypolicyviolation", (event) => {
      console.error(
        `[CSP violation] ${event.violatedDirective}: ${event.blockedURI}`,
      );
    });
  });
  await installMockIpc(page);
});

test.afterEach(async ({ page }) => {
  expect(cspViolations.get(page) ?? []).toEqual([]);
});

test("creates, edits, and reorders a playbook", async ({ page }) => {
  await openCadence(page, "/");

  await page.getByRole("button", { name: "New Playbook" }).click();
  let builder = page
    .getByRole("heading", { name: "New Playbook" })
    .locator("xpath=../..");
  await builder.getByLabel("Playbook name").fill("Release Workflow");
  await builder.getByLabel("Playbook description").fill("Prepare a release");

  await builder.getByRole("button", { name: "+ Single" }).click();
  await builder
    .getByRole("button", { name: "Alpha Prompt", exact: true })
    .click();
  await builder.getByLabel("Step 1 instructions").fill("Draft the notes");

  await builder.getByRole("button", { name: "+ Single" }).click();
  await builder
    .getByRole("button", { name: "Beta Prompt", exact: true })
    .last()
    .click();
  await builder.getByLabel("Step 2 instructions").fill("Review the result");
  await builder.getByRole("button", { name: "Save Playbook" }).click();

  await expect(
    page.getByRole("heading", { name: "Release Workflow" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Edit", exact: true }).click();

  builder = page
    .getByRole("heading", { name: "Edit Playbook" })
    .locator("xpath=../..");
  await builder.getByLabel("Playbook name").fill("Release Workflow Edited");
  await builder.getByRole("button", { name: "Move step 2 up" }).click();
  await builder.getByRole("button", { name: "Save Playbook" }).click();

  await expect(
    page.getByRole("heading", { name: "Release Workflow Edited" }),
  ).toBeVisible();

  const mutationCalls = await ipcCalls(page, [
    "create_playbook",
    "add_step",
    "update_playbook",
    "reorder_steps",
  ]);
  expect(
    mutationCalls.find((call) => call.command === "create_playbook")?.args,
  ).toMatchObject({
    title: "Release Workflow",
    description: "Prepare a release",
  });
  expect(
    mutationCalls.filter((call) => call.command === "add_step"),
  ).toHaveLength(2);
  expect(
    mutationCalls.find((call) => call.command === "update_playbook")?.args,
  ).toMatchObject({
    id: "playbook-1",
    request: { title: "Release Workflow Edited" },
  });
  expect(
    mutationCalls.find((call) => call.command === "reorder_steps")?.args,
  ).toEqual({
    playbookId: "playbook-1",
    orderedStepIds: ["step-2", "step-1"],
  });
});

test("search palette copies the keyboard-selected result and closes", async ({
  page,
}) => {
  await openCadence(page, "/search.html");

  const search = page.getByPlaceholder("Search prompts...");
  await search.fill("prompt");
  await expect(page.getByText("Alpha Prompt", { exact: true }).first()).toBeVisible();
  await search.press("ArrowDown");
  await expect(page.getByText("Beta content", { exact: true })).toBeVisible();
  await search.press("Enter");

  await expect
    .poll(async () => {
      const calls = await ipcCalls(page, [
        "plugin:clipboard-manager|write_text",
        "record_copy",
        "hide_search_window",
      ]);
      return calls.map((call) => call.command);
    })
    .toEqual(
      expect.arrayContaining([
        "plugin:clipboard-manager|write_text",
        "record_copy",
        "hide_search_window",
      ]),
    );

  const calls = await ipcCalls(page, [
    "plugin:clipboard-manager|write_text",
    "record_copy",
  ]);
  expect(
    calls.find(
      (call) => call.command === "plugin:clipboard-manager|write_text",
    )?.args,
  ).toEqual({ text: "Beta content" });
  expect(calls.find((call) => call.command === "record_copy")?.args).toEqual({
    promptId: "prompt-beta",
    variantId: "variant-beta",
  });
});

test("imports JSON and Markdown and renders per-item errors", async ({
  page,
}) => {
  await openCadence(page, "/");

  await page.getByRole("button", { name: "Import", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Import Prompts" });
  const jsonInput = dialog.getByPlaceholder("Paste JSON here...");

  await jsonInput.fill("{");
  await expect(dialog.getByText("Invalid JSON")).toBeVisible();
  await jsonInput.fill('{"prompts":[{"title":"Imported"}]}');
  await expect(dialog.getByText("Found 1 prompt")).toBeVisible();
  await dialog.getByRole("button", { name: "Import", exact: true }).click();
  await expect(dialog.getByText("Imported 1")).toBeVisible();
  await expect(dialog.getByText("Skipped 1")).toBeVisible();
  await expect(dialog.getByText("Duplicate prompt skipped")).toBeVisible();

  await dialog.getByRole("button", { name: "Markdown" }).click();
  await dialog.locator('input[type="file"][accept*=".md"]').setInputFiles({
    name: "release-notes.md",
    mimeType: "text/markdown",
    buffer: Buffer.from("# Release notes"),
  });
  await expect(dialog.getByText("1 file selected")).toBeVisible();
  await dialog.getByRole("button", { name: "Import", exact: true }).click();
  await expect(dialog.getByText("All prompts imported successfully")).toBeVisible();

  const importCalls = await ipcCalls(page, [
    "import_json",
    "import_markdown_files",
  ]);
  expect(importCalls.map((call) => call.command)).toEqual([
    "import_json",
    "import_markdown_files",
  ]);
  expect(importCalls[1].args).toEqual({
    files: [["release-notes.md", "# Release notes"]],
  });
});

test("preserves prompt drafts across a db-changed refetch", async ({ page }) => {
  await openCadence(page, "/");

  await page.getByText("Alpha Prompt", { exact: true }).first().click();
  await page.getByRole("button", { name: /^Edit/ }).click();
  await page.getByLabel("Prompt title").fill("Local Draft Title");
  await page.getByLabel("Variant content").fill("Local draft content");

  await page.evaluate(() => {
    const bridge = window.__CADENCE_E2E__;
    if (!bridge) throw new Error("Cadence E2E bridge is not installed");
    bridge.replacePrompt("prompt-alpha", {
      title: "Server Refetched Title",
      content: "Server refetched content",
    });
  });

  await expect(
    page.getByText("Server Refetched Title", { exact: true }),
  ).toBeVisible();
  await expect(page.getByLabel("Prompt title")).toHaveValue("Local Draft Title");
  await expect(page.getByLabel("Variant content")).toHaveValue(
    "Local draft content",
  );

  await page.getByRole("button", { name: /^Save/ }).click();
  await expect(
    page.getByRole("heading", { name: "Local Draft Title" }),
  ).toBeVisible();

  const saveCalls = await ipcCalls(page, ["update_prompt", "update_variant"]);
  expect(saveCalls.find((call) => call.command === "update_prompt")?.args).toEqual({
    id: "prompt-alpha",
    request: {
      title: "Local Draft Title",
      description: "Alpha description",
    },
  });
  expect(saveCalls.find((call) => call.command === "update_variant")?.args).toEqual({
    id: "variant-alpha",
    content: "Local draft content",
    label: "Default",
  });
});

test("enables and disables the local API from Settings", async ({ page }) => {
  await openCadence(page, "/");

  await page.getByRole("button", { name: "Settings" }).click();
  const dialog = page.getByRole("dialog", { name: "Settings" });
  const toggle = dialog.getByRole("switch", { name: "Enable local API" });

  await expect(toggle).toBeEnabled();
  await expect(toggle).not.toBeChecked();
  await toggle.click();
  await expect(toggle).toBeChecked();
  await toggle.click();
  await expect(toggle).not.toBeChecked();

  const calls = await ipcCalls(page, ["set_api_enabled"]);
  expect(calls.map((call) => call.args)).toEqual([
    { enabled: true },
    { enabled: false },
  ]);
});

interface IpcCall {
  command: string;
  args: Record<string, unknown>;
}

declare global {
  interface Window {
    __CADENCE_E2E__?: {
      calls: IpcCall[];
      emit: (event: string, payload?: unknown) => void;
      invoke: (
        command: string,
        args: Record<string, unknown>,
      ) => unknown;
      replacePrompt: (
        promptId: string,
        update: { title?: string; content?: string },
      ) => void;
    };
  }
}

async function openCadence(page: Page, path: string) {
  const response = await page.goto(path);
  expect(response).not.toBeNull();
  expect(response?.ok()).toBe(true);
  expect(response?.headers()["content-security-policy"]).toBe(PINNED_CSP);
}

async function ipcCalls(page: Page, commands: string[]): Promise<IpcCall[]> {
  return page.evaluate((allowedCommands) => {
    return (window.__CADENCE_E2E__?.calls ?? []).filter((call) =>
      allowedCommands.includes(call.command),
    );
  }, commands);
}

async function installMockIpc(page: Page) {
  await page.addInitScript(() => {
    const clone = <T,>(value: T): T =>
      JSON.parse(JSON.stringify(value)) as T;
    const timestamp = "2026-08-02T12:00:00Z";
    const prompts = [
      {
        id: "prompt-alpha",
        title: "Alpha Prompt",
        description: "Alpha description",
        primary_variant_id: "variant-alpha",
        is_favorite: false,
        is_pinned: false,
        copy_count: 0,
        last_copied_at: null,
        created_at: timestamp,
        updated_at: timestamp,
        variants: [
          {
            id: "variant-alpha",
            prompt_id: "prompt-alpha",
            label: "Default",
            content: "Alpha content",
            content_type: "text",
            variables: null,
            sort_order: 0,
            created_at: timestamp,
            updated_at: timestamp,
          },
        ],
        tags: [{ id: "tag-writing", name: "writing", color: "#34c759" }],
      },
      {
        id: "prompt-beta",
        title: "Beta Prompt",
        description: "Beta description",
        primary_variant_id: "variant-beta",
        is_favorite: true,
        is_pinned: false,
        copy_count: 2,
        last_copied_at: timestamp,
        created_at: timestamp,
        updated_at: timestamp,
        variants: [
          {
            id: "variant-beta",
            prompt_id: "prompt-beta",
            label: "Default",
            content: "Beta content",
            content_type: "text",
            variables: null,
            sort_order: 0,
            created_at: timestamp,
            updated_at: timestamp,
          },
        ],
        tags: [{ id: "tag-support", name: "support", color: "#ff9500" }],
      },
    ];
    const state = {
      prompts,
      apiEnabled: false,
      playbooks: [] as Array<{
        id: string;
        title: string;
        description: string | null;
        steps: Array<{
          id: string;
          playbook_id: string;
          prompt_id: string | null;
          position: number;
          step_type: "single" | "choice";
          instructions: string | null;
          choice_prompt_ids: string[];
        }>;
      }>,
      nextPlaybook: 1,
      nextStep: 1,
    };
    const calls: IpcCall[] = [];
    const listeners = new Map<
      number,
      { event: string; callbackId: number }
    >();
    let nextListenerId = 1;

    function listItem(prompt: (typeof prompts)[number]) {
      const primary =
        prompt.variants.find(
          (variant) => variant.id === prompt.primary_variant_id,
        ) ?? prompt.variants[0];
      return {
        id: prompt.id,
        title: prompt.title,
        description: prompt.description,
        snippet: primary?.content ?? "",
        snippet_runs: [],
        is_favorite: prompt.is_favorite,
        variant_count: prompt.variants.length,
        copy_count: prompt.copy_count,
        last_copied_at: prompt.last_copied_at,
        tags: clone(prompt.tags),
      };
    }

    function findPrompt(promptId: string) {
      const prompt = state.prompts.find((candidate) => candidate.id === promptId);
      if (!prompt) throw new Error(`Prompt not found: ${promptId}`);
      return prompt;
    }

    function findPlaybook(playbookId: string) {
      const playbook = state.playbooks.find(
        (candidate) => candidate.id === playbookId,
      );
      if (!playbook) throw new Error(`Playbook not found: ${playbookId}`);
      return playbook;
    }

    function hydrateStep(
      step: (typeof state.playbooks)[number]["steps"][number],
    ) {
      return {
        ...clone(step),
        prompt: step.prompt_id ? clone(findPrompt(step.prompt_id)) : null,
        choice_prompts: step.choice_prompt_ids.map((id) =>
          clone(findPrompt(id)),
        ),
      };
    }

    function emit(event: string, payload: unknown = null) {
      for (const [eventId, listener] of listeners) {
        if (listener.event !== event) continue;
        const callback = window[
          `_${listener.callbackId}` as keyof Window
        ] as ((event: { event: string; id: number; payload: unknown }) => void)
          | undefined;
        callback?.({ event, id: eventId, payload });
      }
    }

    function dbChanged() {
      emit("db-changed");
    }

    const bridge = {
      calls,
      emit,
      replacePrompt(
        promptId: string,
        update: { title?: string; content?: string },
      ) {
        const prompt = findPrompt(promptId);
        if (update.title !== undefined) prompt.title = update.title;
        if (update.content !== undefined) {
          const primary =
            prompt.variants.find(
              (variant) => variant.id === prompt.primary_variant_id,
            ) ?? prompt.variants[0];
          if (primary) primary.content = update.content;
        }
        dbChanged();
      },
      invoke(command: string, args: Record<string, unknown>) {
        calls.push({ command, args: clone(args) });

        switch (command) {
          case "plugin:event|listen": {
            const eventId = nextListenerId++;
            listeners.set(eventId, {
              event: String(args.event),
              callbackId: Number(args.handler),
            });
            return eventId;
          }
          case "plugin:event|unlisten":
            listeners.delete(Number(args.eventId));
            return null;
          case "list_prompts": {
            const filter = args.filter;
            const offset = Number(args.offset ?? 0);
            const limit = Number(args.limit ?? 100);
            let filtered = state.prompts;
            if (filter === "favorites") {
              filtered = filtered.filter((prompt) => prompt.is_favorite);
            } else if (filter === "recent") {
              filtered = filtered.filter((prompt) => prompt.last_copied_at);
            }
            return clone(filtered.slice(offset, offset + limit).map(listItem));
          }
          case "get_prompt_counts":
            return {
              all: state.prompts.length,
              favorites: state.prompts.filter((prompt) => prompt.is_favorite)
                .length,
              recents: state.prompts.filter((prompt) => prompt.last_copied_at)
                .length,
            };
          case "get_prompt":
            return clone(findPrompt(String(args.id)));
          case "search_prompts": {
            const query = String(args.query).toLocaleLowerCase();
            return clone(
              state.prompts
                .filter((prompt) => {
                  return [
                    prompt.title,
                    prompt.description ?? "",
                    ...prompt.variants.map((variant) => variant.content),
                    ...prompt.tags.map((tag) => tag.name),
                  ].some((value) => value.toLocaleLowerCase().includes(query));
                })
                .map(listItem),
            );
          }
          case "list_collections":
            return [];
          case "list_tags":
            return clone(state.prompts.flatMap((prompt) => prompt.tags));
          case "get_playbook_session":
            return {
              active_playbook_id: null,
              current_step: 0,
              started_at: null,
            };
          case "get_keyboard_shortcuts":
            return [];
          case "get_api_enabled":
            return state.apiEnabled;
          case "set_api_enabled":
            state.apiEnabled = Boolean(args.enabled);
            dbChanged();
            return {
              enabled: state.apiEnabled,
              port: state.apiEnabled ? 41_237 : null,
            };
          case "list_playbooks":
            return clone(
              state.playbooks.map(({ id, title, description }) => ({
                id,
                title,
                description,
              })),
            );
          case "get_playbook": {
            const playbook = findPlaybook(String(args.id));
            return {
              id: playbook.id,
              title: playbook.title,
              description: playbook.description,
              steps: playbook.steps.map(hydrateStep),
            };
          }
          case "create_playbook": {
            const playbook = {
              id: `playbook-${state.nextPlaybook++}`,
              title: String(args.title),
              description:
                args.description === undefined
                  ? null
                  : String(args.description),
              steps: [],
            };
            state.playbooks.push(playbook);
            dbChanged();
            return clone(playbook);
          }
          case "update_playbook": {
            const playbook = findPlaybook(String(args.id));
            const request = args.request as {
              title?: string;
              description?: string | null;
            };
            if (request.title !== undefined) playbook.title = request.title;
            if (request.description !== undefined) {
              playbook.description = request.description;
            }
            dbChanged();
            return clone(playbook);
          }
          case "add_step": {
            const playbook = findPlaybook(String(args.playbookId));
            const spec = args.spec as {
              step_type: "single" | "choice";
              prompt_id: string | null;
              choice_prompt_ids: string[];
              instructions: string | null;
            };
            const step = {
              id: `step-${state.nextStep++}`,
              playbook_id: playbook.id,
              prompt_id: spec.prompt_id,
              position: playbook.steps.length,
              step_type: spec.step_type,
              instructions: spec.instructions,
              choice_prompt_ids: clone(spec.choice_prompt_ids),
            };
            playbook.steps.push(step);
            dbChanged();
            return hydrateStep(step);
          }
          case "update_step": {
            const playbook = findPlaybook(String(args.playbookId));
            const step = playbook.steps.find(
              (candidate) => candidate.id === String(args.stepId),
            );
            if (!step) throw new Error("Step not found");
            const spec = args.spec as {
              step_type: "single" | "choice";
              prompt_id: string | null;
              choice_prompt_ids: string[];
              instructions: string | null;
            };
            Object.assign(step, clone(spec));
            dbChanged();
            return hydrateStep(step);
          }
          case "remove_step": {
            const playbook = findPlaybook(String(args.playbookId));
            playbook.steps = playbook.steps.filter(
              (step) => step.id !== String(args.stepId),
            );
            playbook.steps.forEach((step, index) => {
              step.position = index;
            });
            dbChanged();
            return null;
          }
          case "reorder_steps": {
            const playbook = findPlaybook(String(args.playbookId));
            const orderedStepIds = args.orderedStepIds as string[];
            playbook.steps.sort(
              (left, right) =>
                orderedStepIds.indexOf(left.id) -
                orderedStepIds.indexOf(right.id),
            );
            playbook.steps.forEach((step, index) => {
              step.position = index;
            });
            dbChanged();
            return null;
          }
          case "update_prompt": {
            const prompt = findPrompt(String(args.id));
            const request = args.request as {
              title?: string;
              description?: string | null;
            };
            if (request.title !== undefined) prompt.title = request.title;
            if (request.description !== undefined) {
              prompt.description = request.description;
            }
            dbChanged();
            return null;
          }
          case "update_variant": {
            const variant = state.prompts
              .flatMap((prompt) => prompt.variants)
              .find((candidate) => candidate.id === String(args.id));
            if (!variant) throw new Error("Variant not found");
            variant.content = String(args.content);
            if (args.label !== undefined) variant.label = String(args.label);
            dbChanged();
            return null;
          }
          case "import_json":
            return {
              imported: 1,
              skipped: 1,
              errors: ["Duplicate prompt skipped"],
            };
          case "import_markdown_files":
            return {
              imported: (args.files as unknown[]).length,
              skipped: 0,
              errors: [],
            };
          case "plugin:clipboard-manager|write_text":
          case "hide_search_window":
            return null;
          case "record_copy": {
            const prompt = findPrompt(String(args.promptId));
            const variant = prompt.variants.find(
              (candidate) => candidate.id === String(args.variantId),
            );
            return variant?.content ?? "";
          }
          default:
            throw new Error(`Unhandled mock IPC command: ${command}`);
        }
      },
    };

    window.__CADENCE_E2E__ = bridge;
  });
}
