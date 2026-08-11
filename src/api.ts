// Thin wrapper over the Tauri command surface.
// Every filesystem and network operation lives in Rust; this file only calls it.

import { invoke } from "@tauri-apps/api/core";
import type {
  Bundle,
  CliKind,
  ImportPlan,
  InstallReport,
  ModelInfo,
  ModelIssue,
  ModelMap,
  Profile,
  ProbeReport,
  ProfileState,
} from "./types";

export const cliState = (cli: CliKind) =>
  invoke<ProfileState>("cli_state", { cli });

export const scanMachine = () => invoke<Profile[]>("scan_machine");

export const parseModelList = (body: string) =>
  invoke<ModelInfo[]>("parse_model_list", { body });

export const validateModelMapping = (
  cli: CliKind,
  map: ModelMap,
  available: ModelInfo[],
) => invoke<ModelIssue[]>("validate_model_mapping", { cli, map, available });

export const suggestModelMapping = (cli: CliKind, available: ModelInfo[]) =>
  invoke<ModelMap>("suggest_model_mapping", { cli, available });

export const planImport = (incoming: Bundle, existing: Profile[]) =>
  invoke<ImportPlan[]>("plan_import", { incoming, existing });

export const installProfiles = (profiles: Profile[]) =>
  invoke<InstallReport>("install_profiles", { profiles });

/** Resolves to true when the alias shadows a common command; rejects when it
 *  cannot be typed at a shell prompt. */
export const validateAlias = (alias: string) =>
  invoke<boolean>("validate_alias", { alias });

export const probeProfile = (profile: Profile) =>
  invoke<ProbeReport>("probe_profile", { profile });

/** Fetches the model list a provider serves to this key.
 *
 *  Runs in the webview rather than Rust so the UI can show progress, but the
 *  parsing still goes through Rust so both sides agree on the shape. */
export async function fetchModels(
  baseUrl: string,
  apiKey: string,
): Promise<ModelInfo[]> {
  const root = baseUrl.replace(/\/+$/, "").replace(/\/v1$/, "");
  const res = await fetch(`${root}/v1/models`, {
    headers: { Authorization: `Bearer ${apiKey}` },
  });
  if (!res.ok) {
    throw new Error(`provider returned ${res.status} for the model list`);
  }
  return parseModelList(await res.text());
}
