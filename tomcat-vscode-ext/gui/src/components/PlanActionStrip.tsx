import { buildPickerModels } from "./buildPickerModels";
import { ModelPicker, type ModelPickerModel } from "./ModelPicker";
import type { PlanFileState } from "../../../src/shared/planPreviewProtocol";

/**
 * Hybrid (B) in-body action strip: the model dropdown plus the yellow Build
 * button, rendered once at the top of the plan content. It carries no file
 * path and no Preview/Markdown toggle (both live on the native title bar) and
 * it does not stick — VS Code's own editor title bar already floats.
 */
export function PlanActionStrip({
  availableModelDetails,
  availableModels,
  buildModel,
  canBuild,
  fileState,
  onBuild,
  onSelectContextWindow,
  onSelectThinkingLevel,
  onSetBuildModel,
  sessionModel,
}: {
  availableModelDetails?: Record<string, ModelPickerModel>;
  availableModels: string[];
  buildModel: string;
  canBuild: boolean;
  fileState: PlanFileState | null;
  onBuild(): void;
  onSelectContextWindow(modelId: string, contextWindow: number): void;
  onSelectThinkingLevel(modelId: string, level: string): void;
  onSetBuildModel(modelId: string): void;
  sessionModel: string;
}) {
  const selectedModelId = buildModel || sessionModel || null;
  const pickerModels = buildPickerModels({
    activeModelId: sessionModel,
    availableModelDetails,
    availableModels,
    selectedModelId,
  });

  return (
    <div className="tc-plan-action-strip" data-testid="plan-action-strip">
      <ModelPicker
        className="tc-plan-model-picker"
        disabled={pickerModels.length === 0}
        label="Build model"
        models={pickerModels}
        onSelectContextWindow={onSelectContextWindow}
        onSelectModel={onSetBuildModel}
        onSelectThinkingLevel={onSelectThinkingLevel}
        placement="below"
        selectedModelId={selectedModelId}
        testId="plan-build-model-select"
      />
      <button
        className="tc-button tc-plan-build-button"
        data-testid="plan-build"
        disabled={!canBuild}
        onClick={onBuild}
        type="button"
      >
        {fileState === "pending" ? "Resume" : "Build"}
      </button>
    </div>
  );
}
