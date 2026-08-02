import { api } from './api';

export interface ToggleSettingDefinition {
  id: string;
  kind: 'toggle';
  section: string;
  label: string;
  description: string;
  read: () => Promise<boolean>;
  write: (enabled: boolean) => Promise<boolean>;
}

export type SettingDefinition = ToggleSettingDefinition;

export const SETTINGS_SCHEMA: readonly SettingDefinition[] = [
  {
    id: 'api_enabled',
    kind: 'toggle',
    section: 'Local API',
    label: 'Enable local API',
    description:
      'Starts an authenticated HTTP server on this Mac. Connection details are written to api.json, and the access key changes each time the server starts.',
    read: api.settings.getApiEnabled,
    write: async (enabled) => {
      const status = await api.settings.setApiEnabled(enabled);
      return status.enabled;
    },
  },
];
