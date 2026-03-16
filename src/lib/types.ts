export interface Prompt {
  id: string;
  title: string;
  description: string | null;
  primary_variant_id: string | null;
  is_favorite: boolean;
  is_pinned: boolean;
  copy_count: number;
  last_copied_at: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface Variant {
  id: string;
  prompt_id: string;
  label: string;
  content: string;
  content_type: string | null;
  variables: string | null;
  sort_order: number;
  created_at: string | null;
  updated_at: string | null;
}

export interface Tag {
  id: string;
  name: string;
  color: string | null;
}

export interface PromptWithVariants extends Prompt {
  variants: Variant[];
  tags: Tag[];
}

export interface PromptListItem {
  id: string;
  title: string;
  description: string | null;
  snippet: string;
  is_favorite: boolean;
  variant_count: number;
  copy_count: number;
  last_copied_at: string | null;
  tags: Tag[];
}

export interface Collection {
  id: string;
  name: string;
  description: string | null;
  icon: string | null;
  color: string | null;
  is_smart: boolean;
  filter_query: string | null;
}

export interface Playbook {
  id: string;
  title: string;
  description: string | null;
}

export interface PlaybookStep {
  id: string;
  playbook_id: string;
  prompt_id: string | null;
  position: number;
  step_type: 'single' | 'choice';
  instructions: string | null;
}

export interface PlaybookWithSteps extends Playbook {
  steps: PlaybookStepWithPrompt[];
}

export interface PlaybookStepWithPrompt extends PlaybookStep {
  prompt?: PromptWithVariants;
  choice_prompts?: PromptWithVariants[];
}

export interface ImportResult {
  imported: number;
  skipped: number;
  errors: string[];
}

export interface PlaybookSession {
  active_playbook_id: string | null;
  current_step: number;
  started_at: string | null;
}

export interface KeyboardShortcut {
  action: string;
  binding: string;
  label: string;
  default_binding: string;
  is_global: boolean;
}
