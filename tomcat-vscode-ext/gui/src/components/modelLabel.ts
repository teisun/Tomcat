export function thinkingLevelLabel(value: string | null | undefined): string {
  const normalized = value?.trim().toLowerCase() ?? "";
  switch (normalized) {
    case "":
      return "";
    case "off":
      return "Off";
    case "xhigh":
      return "Xhigh";
    default:
      return titleCaseToken(normalized);
  }
}

/**
 * Renders the single compact label used by all model-picker triggers.
 *
 * A reasoning suffix is deliberately shown only when the model declares one
 * or more selectable reasoning tiers and a selected tier is available. This
 * keeps plain models and incomplete state snapshots from pretending to have a
 * reasoning setting.
 */
export function formatModelLabel({
  modelId,
  selectedReasoningLevel,
  supportedReasoningLevels,
}: {
  modelId: string | null | undefined;
  selectedReasoningLevel?: string | null;
  supportedReasoningLevels?: readonly string[] | null;
}): string {
  const { id, reasoning } = modelLabelParts({
    modelId,
    selectedReasoningLevel,
    supportedReasoningLevels,
  });
  return reasoning ? `${id} ${reasoning}` : id;
}

export function modelLabelParts({
  modelId,
  selectedReasoningLevel,
  supportedReasoningLevels,
}: {
  modelId: string | null | undefined;
  selectedReasoningLevel?: string | null;
  supportedReasoningLevels?: readonly string[] | null;
}): { id: string; reasoning: string } {
  const id = modelId?.trim() ?? "";
  if (!id) {
    return { id: "Model", reasoning: "" };
  }
  if (!supportedReasoningLevels?.some((level) => level.trim())) {
    return { id, reasoning: "" };
  }
  const reasoning = thinkingLevelLabel(selectedReasoningLevel);
  return { id, reasoning };
}

function titleCaseToken(value: string): string {
  return value ? value[0].toUpperCase() + value.slice(1) : "";
}
