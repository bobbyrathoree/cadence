import { invoke } from '@tauri-apps/api/core';
import type * as T from './types';

export const api = {
  prompts: {
    list: () => invoke<T.PromptListItem[]>('list_prompts'),
    get: (id: string) => invoke<T.PromptWithVariants>('get_prompt', { id }),
    create: (request: { title: string; content: string; tags?: string[]; description?: string }) =>
      invoke<T.PromptWithVariants>('create_prompt', { request }),
    update: (id: string, request: Record<string, unknown>) =>
      invoke('update_prompt', { id, request }),
    delete: (id: string) => invoke('delete_prompt', { id }),
    toggleFavorite: (id: string) => invoke<boolean>('toggle_favorite', { id }),
    recordCopy: (promptId: string, variantId?: string) =>
      invoke<string>('record_copy', { promptId, variantId }),
  },
  variants: {
    add: (promptId: string, label: string, content: string) =>
      invoke<T.Variant>('add_variant', { promptId, label, content }),
    update: (id: string, content: string, label?: string) =>
      invoke('update_variant', { id, content, label }),
    delete: (id: string) => invoke('delete_variant', { id }),
  },
  tags: {
    list: () => invoke<T.Tag[]>('list_tags'),
    create: (request: { name: string; color?: string }) =>
      invoke<T.Tag>('create_tag', { request }),
    addToPrompt: (promptId: string, tags: string[]) =>
      invoke<T.Tag[]>('add_tags_to_prompt', { promptId, tags }),
    removeFromPrompt: (promptId: string, tagId: string) =>
      invoke('remove_tag_from_prompt', { promptId, tagId }),
  },
  collections: {
    list: () => invoke<T.Collection[]>('list_collections'),
    create: (request: { name: string; description?: string; is_smart?: boolean; filter_query?: string }) =>
      invoke<T.Collection>('create_collection', { request }),
    getPrompts: (collectionId: string) =>
      invoke<T.PromptListItem[]>('get_collection_prompts', { collectionId }),
  },
  search: (query: string) => invoke<T.PromptListItem[]>('search_prompts', { query }),
  playbooks: {
    list: () => invoke<T.Playbook[]>('list_playbooks'),
    get: (id: string) => invoke<T.PlaybookWithSteps>('get_playbook', { id }),
    create: (title: string, description?: string) =>
      invoke<T.Playbook>('create_playbook', { title, description }),
    addStep: (playbookId: string, opts: { promptId?: string; stepType: string; instructions?: string; choicePromptIds?: string[] }) =>
      invoke<T.PlaybookStep>('add_playbook_step', { playbookId, ...opts }),
  },
  session: {
    get: () => invoke<T.PlaybookSession>('get_playbook_session'),
    start: (playbookId: string) => invoke<T.PlaybookSession>('start_playbook_session', { playbookId }),
    advance: () => invoke<T.PlaybookSession>('advance_playbook_step'),
    end: () => invoke('end_playbook_session'),
  },
};
