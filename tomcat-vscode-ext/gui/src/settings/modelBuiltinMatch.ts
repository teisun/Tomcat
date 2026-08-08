import type { SettingsModelView } from "../../../src/shared/settingsProtocol";

/**
 * Finds the built-in specification for a provider-facing model name.
 *
 * This intentionally differs from the write-collision check: a relay model
 * may use a different Tomcat id but still refer to the same upstream model.
 * Catalog order is preserved so duplicate built-in names have one stable,
 * explainable winner.
 */
export function findBuiltinModelByName(
  models: readonly SettingsModelView[],
  modelName: string | null | undefined,
): SettingsModelView | null {
  const normalizedName = modelName?.trim();
  if (!normalizedName) return null;
  return (
    models.find(
      (model) =>
        model.source === "builtin" &&
        (model.modelName?.trim() || model.id.trim()) === normalizedName,
    ) ?? null
  );
}
